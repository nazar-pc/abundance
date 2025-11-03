#[cfg(all(test, not(target_arch = "spirv")))]
mod tests;

use crate::shader::constants::{MAX_BUCKET_SIZE, PARAM_BC, REDUCED_BUCKET_SIZE};
#[cfg(target_arch = "spirv")]
use crate::shader::find_matches_in_buckets::ArrayIndexingPolyfill;
use crate::shader::types::{Position, PositionExt, PositionR, R};
use core::mem::MaybeUninit;
use spirv_std::arch::{atomic_or, subgroup_exclusive_i_add, subgroup_shuffle, subgroup_u_max};
use spirv_std::memory::{Scope, Semantics};

// TODO: Benchmark on different GPUs to see if the complexity of dealing with 9-bit pointers is
//  worth it or maybe using u16s would be better despite using more shared memory
/// Number of bits necessary to address a single pair of positions in the rmap.
///
/// `+1` is used because virtual pointers used for storage are increased by `1`
const VIRTUAL_POINTER_BITS: u32 = (REDUCED_BUCKET_SIZE + 1).bit_width();
const POINTERS_BITS: usize = PARAM_BC as usize * VIRTUAL_POINTER_BITS as usize;
const POINTERS_WORDS: usize = POINTERS_BITS.div_ceil(u32::BITS as usize);

// Ensure `u32` is large enough as a container for pointers
const _: () = assert!(VIRTUAL_POINTER_BITS <= u32::BITS);

#[cfg(test)]
#[derive(Debug, Default)]
pub(super) struct NextPhysicalPointer {
    next_physical_pointer: u32,
}

#[cfg(test)]
impl NextPhysicalPointer {
    /// Increments next physical pointer and returns previous value
    #[inline(always)]
    fn inc(&mut self) -> u32 {
        let physical_pointer = self.next_physical_pointer;
        self.next_physical_pointer += 1;
        physical_pointer
    }
}

// TODO: The struct in this form currently doesn't compile:
//  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
// #[derive(Debug, Copy, Clone)]
// #[repr(C)]
// pub(super) struct RmapBitPosition(u32);
//
// impl RmapBitPosition {
//     /// # Safety
//     /// `r` must be in the range `0..PARAM_BC`
//     #[inline(always)]
//     pub(super) unsafe fn new(r: u32) -> Self {
//         Self(r * VIRTUAL_POINTER_BITS)
//     }
//
//     /// Extract `rmap_bit_position` out of the inner value
//     #[inline(always)]
//     fn get(self) -> u32 {
//         self.0
//     }
// }

pub(in super::super) type RmapBitPosition = u32;

// TODO: Remove once normal `RmapBitPosition` struct can be used
pub(in super::super) trait RmapBitPositionExt: Sized {
    /// # Safety
    /// `r` must be in the range `0..PARAM_BC`
    unsafe fn new(r: u32) -> Self;

    /// Extract `rmap_bit_position` out of the inner value
    fn get(self) -> u32;
}

impl RmapBitPositionExt for RmapBitPosition {
    /// # Safety
    /// `r` must be in the range `0..PARAM_BC`
    #[inline(always)]
    unsafe fn new(r: u32) -> Self {
        r * VIRTUAL_POINTER_BITS
    }

    /// Extract `rmap_bit_position` out of the inner value
    #[inline(always)]
    fn get(self) -> u32 {
        self
    }
}

enum ConcurrentAddSlot {
    First { physical_pointer: u32 },
    Second { physical_pointer: u32 },
    Ignore,
}

impl ConcurrentAddSlot {
    #[inline(always)]
    fn into_data(self) -> u32 {
        match self {
            Self::First { physical_pointer } => physical_pointer << 2,
            Self::Second { physical_pointer } => (physical_pointer << 2) | 1,
            Self::Ignore => 0b11,
        }
    }

    /// SAFETY:
    /// `data` must be created from the [`Self::First`] variant before
    #[inline(always)]
    unsafe fn data_to_second(data: u32) -> u32 {
        data | 1
    }

    #[inline(always)]
    fn is_first(data: u32) -> bool {
        data & 0b11 == 0
    }

    #[inline(always)]
    fn is_ignore(data: u32) -> bool {
        data == 0b11
    }

    /// # Safety
    /// Must be called with data created from [`Self::into_data()`]. Note that this result is only
    /// meaningful for [`Self::First`] and [`Self::Second`].
    #[inline(always)]
    unsafe fn positions_offset_and_physical_pointer_from_data(data: u32) -> (usize, u32) {
        (data as usize & 0b11, data >> 2)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Rmap {
    /// `0` is a sentinel value indicating no virtual pointer is stored yet.
    ///
    /// Physical pointer must be increased by `1` to get a virtual pointer before storing. Virtual
    /// pointer must be decreased by `1` before reading to get a physical pointer.
    virtual_pointers: [u32; POINTERS_WORDS],
    positions: [[Position; 2]; REDUCED_BUCKET_SIZE],
}

impl Rmap {
    #[cfg(test)]
    #[inline(always)]
    pub(in super::super) fn new() -> Self {
        Self {
            virtual_pointers: [0; _],
            positions: [[Position::SENTINEL; 2]; _],
        }
    }

    /// # Safety
    /// There must be at most [`REDUCED_BUCKET_SIZE`] items inserted. `NextPhysicalPointer` and
    /// `Rmap` must have 1:1 mapping and not mixed with anything else.
    #[cfg(test)]
    #[inline(always)]
    fn insertion_item_physical_pointer(
        &mut self,
        rmap_bit_position: RmapBitPosition,
        next_physical_pointer: &mut NextPhysicalPointer,
    ) -> u32 {
        let bit_position = rmap_bit_position.get();
        let word_offset = (bit_position / u32::BITS) as usize;
        let bit_offset = bit_position % u32::BITS;

        // SAFETY: Offset comes from `RmapBitPosition`, whose constructor guarantees bounds
        let mut word = *unsafe { self.virtual_pointers.get_unchecked_mut(word_offset) };

        if bit_offset + VIRTUAL_POINTER_BITS > u32::BITS {
            // SAFETY: Offset comes from `RmapBitPosition`, whose constructor guarantees bounds
            let mut word_next =
                *unsafe { self.virtual_pointers.get_unchecked_mut(word_offset + 1) };
            {
                let value = (word >> bit_offset) | (word_next << (u32::BITS - bit_offset));
                let virtual_pointer = value & (u32::MAX >> (u32::BITS - VIRTUAL_POINTER_BITS));

                if let Some(physical_pointer) = virtual_pointer.checked_sub(1) {
                    return physical_pointer;
                }
            }

            let physical_pointer = next_physical_pointer.inc();
            let virtual_pointer = physical_pointer + 1;

            word |= virtual_pointer << bit_offset;
            word_next |= virtual_pointer >> (u32::BITS - bit_offset);

            // SAFETY: Offset comes from `RmapBitPosition`, whose constructor guarantees bounds
            *unsafe { self.virtual_pointers.get_unchecked_mut(word_offset) } = word;
            // SAFETY: Offset comes from `RmapBitPosition`, whose constructor guarantees bounds
            *unsafe { self.virtual_pointers.get_unchecked_mut(word_offset + 1) } = word_next;

            physical_pointer
        } else {
            {
                let virtual_pointer =
                    (word >> bit_offset) & (u32::MAX >> (u32::BITS - VIRTUAL_POINTER_BITS));

                if let Some(physical_pointer) = virtual_pointer.checked_sub(1) {
                    return physical_pointer;
                }
            }

            let physical_pointer = next_physical_pointer.inc();
            let virtual_pointer = physical_pointer + 1;

            word |= virtual_pointer << bit_offset;

            // SAFETY: Offset comes from `RmapBitPosition`, whose constructor guarantees bounds
            *unsafe { self.virtual_pointers.get_unchecked_mut(word_offset) } = word;

            physical_pointer
        }
    }

    /// Note that `position == Position::ZERO` is effectively ignored here, supporting it cost too
    /// much in terms of performance and not required for correctness.
    ///
    /// # Safety
    /// There must be at most [`REDUCED_BUCKET_SIZE`] items inserted. `NextPhysicalPointer` and
    /// `Rmap` must have 1:1 mapping and not mixed with anything else.
    #[cfg(test)]
    #[inline(always)]
    pub(super) unsafe fn add(
        &mut self,
        rmap_bit_position: RmapBitPosition,
        position: Position,
        next_physical_pointer: &mut NextPhysicalPointer,
    ) {
        let physical_pointer =
            self.insertion_item_physical_pointer(rmap_bit_position, next_physical_pointer);
        // SAFETY: Internal pointers are always valid
        let rmap_item = unsafe { self.positions.get_unchecked_mut(physical_pointer as usize) };

        // The same `r` can appear in the table multiple times, one duplicate is supported here
        if rmap_item[0] == Position::SENTINEL {
            rmap_item[0] = position;
        } else if rmap_item[1] == Position::SENTINEL {
            rmap_item[1] = position;
        }
    }

    /// Add using `r` that was previously updated using [`Self::update_local_bucket_r_data()`] in a
    /// fully parallel way (all threads can contribute at once).
    ///
    /// # Safety
    /// `r` elements must be updated using [`Self::update_local_bucket_r_data()`].
    pub(in super::super) unsafe fn add_with_data_parallel(&mut self, r: R, position: Position) {
        let (r, data) = r.split();
        if ConcurrentAddSlot::is_ignore(data) {
            // This is a duplicate that should be ignored
            return;
        }
        // SAFETY: Guaranteed by function contract
        let (positions_offset, physical_pointer) =
            unsafe { ConcurrentAddSlot::positions_offset_and_physical_pointer_from_data(data) };
        let virtual_pointer = physical_pointer + 1;

        // SAFETY: `r` is obtained from the `R` instance and thus must be valid
        let rmap_bit_position = unsafe { RmapBitPosition::new(r) };

        let bit_position = rmap_bit_position.get();
        let word_offset = (bit_position / u32::BITS) as usize;
        let bit_offset = bit_position % u32::BITS;

        // SAFETY: Offset comes from `RmapBitPosition`, whose constructor guarantees bounds
        let word = unsafe { self.virtual_pointers.get_unchecked_mut(word_offset) };
        if cfg!(target_arch = "spirv") {
            // TODO: Probably should not be unsafe to begin with:
            //  https://github.com/Rust-GPU/rust-gpu/pull/394#issuecomment-3316594485
            unsafe {
                atomic_or::<_, { Scope::Workgroup as u32 }, { Semantics::NONE.bits() }>(
                    word,
                    virtual_pointer << bit_offset,
                );
            }
        } else {
            *word |= virtual_pointer << bit_offset;
        }

        if bit_offset + VIRTUAL_POINTER_BITS > u32::BITS {
            // SAFETY: Offset comes from `RmapBitPosition`, whose constructor guarantees bounds
            let word_next = unsafe { self.virtual_pointers.get_unchecked_mut(word_offset + 1) };
            // TODO: Probably should not be unsafe to begin with:
            //  https://github.com/Rust-GPU/rust-gpu/pull/394#issuecomment-3316594485
            if cfg!(target_arch = "spirv") {
                unsafe {
                    atomic_or::<_, { Scope::Workgroup as u32 }, { Semantics::NONE.bits() }>(
                        word_next,
                        virtual_pointer >> (u32::BITS - bit_offset),
                    );
                }
            } else {
                *word_next |= virtual_pointer >> (u32::BITS - bit_offset);
            }
        }

        let physical_pointer = virtual_pointer - 1;

        // SAFETY: Internal pointers are always valid
        let rmap_item = unsafe { self.positions.get_unchecked_mut(physical_pointer as usize) };
        rmap_item[positions_offset] = position;
    }

    #[inline(always)]
    pub(in super::super) fn get(&self, rmap_bit_position: RmapBitPosition) -> [Position; 2] {
        let bit_position = rmap_bit_position.get();
        let word_offset = (bit_position / u32::BITS) as usize;
        let bit_offset = bit_position % u32::BITS;

        let virtual_pointer = if bit_offset + VIRTUAL_POINTER_BITS > u32::BITS {
            // SAFETY: Offset comes from `RmapBitPosition`, whose constructor guarantees bounds
            let word = unsafe { *self.virtual_pointers.get_unchecked(word_offset) };
            // SAFETY: Offset comes from `RmapBitPosition`, whose constructor guarantees bounds
            let word_next = unsafe { *self.virtual_pointers.get_unchecked(word_offset + 1) };

            let value = (word >> bit_offset) | (word_next << (u32::BITS - bit_offset));
            value & (u32::MAX >> (u32::BITS - VIRTUAL_POINTER_BITS))
        } else {
            // SAFETY: Offset comes from `RmapBitPosition`, whose constructor guarantees bounds
            let word = unsafe { *self.virtual_pointers.get_unchecked(word_offset) };

            (word >> bit_offset) & (u32::MAX >> (u32::BITS - VIRTUAL_POINTER_BITS))
        };

        if let Some(physical_pointer) = virtual_pointer.checked_sub(1) {
            // SAFETY: Internal pointers are always valid
            *unsafe { self.positions.get_unchecked(physical_pointer as usize) }
        } else {
            [Position::SENTINEL; 2]
        }
    }

    #[inline(always)]
    pub(super) fn reset(
        rmap: &mut MaybeUninit<Self>,
        local_invocation_id: u32,
        workgroup_size: u32,
    ) {
        let rmap = unsafe { rmap.assume_init_mut() };

        for i in (local_invocation_id..POINTERS_WORDS as u32).step_by(workgroup_size as usize) {
            rmap.virtual_pointers[i as usize] = 0;
        }

        const {
            assert!(REDUCED_BUCKET_SIZE.is_multiple_of(2));
        }
        let pair_id = local_invocation_id / 2;
        let pair_offset = local_invocation_id % 2;
        for bucket in (pair_id..REDUCED_BUCKET_SIZE as u32).step_by((workgroup_size / 2) as usize) {
            rmap.positions[bucket as usize][pair_offset as usize] = Position::SENTINEL;
        }
    }

    /// This prepares the shared bucket by appending extra information in `R`'s data field, such
    /// that `Rmap` can later be constructed in parallel rather than sequentially.
    ///
    /// NOTE: For this to work correctly, the entire `shared_bucket` must be sorted by `r` and
    /// `position` among `r` duplicates. `r` must not store additional data in it yet.
    ///
    /// # Safety
    /// There must be at most [`REDUCED_BUCKET_SIZE`] non-sentinel values contiguously from the
    /// start of the bucket. This function must be called from a single subgroup with size
    /// `subgroup_size` that is at most [`MAX_BUCKET_SIZE`]. `lane_id` is less
    /// than `subgroup_size`. CPU is also supported, in which case `lane_id == 0` and
    /// `subgroup_size == 1` are expected.
    #[inline]
    pub(in super::super) unsafe fn update_local_bucket_r_data(
        lane_id: u32,
        subgroup_size: u32,
        shared_bucket: &mut [PositionR; MAX_BUCKET_SIZE],
    ) {
        let mut positions_cursor = 0u32;

        if cfg!(target_arch = "spirv") {
            // Must be able to do `-2` lane offset
            debug_assert!(subgroup_size >= 3);
            debug_assert!(MAX_BUCKET_SIZE.is_multiple_of(subgroup_size as usize));

            let mut prev_iter_r_inner = u32::MAX;
            for local_offset in (lane_id as usize..MAX_BUCKET_SIZE).step_by(subgroup_size as usize)
            {
                let position = shared_bucket[local_offset].position;
                let r_inner = shared_bucket[local_offset].r.get_inner();
                let ignore = position == Position::SENTINEL;
                // SAFETY: `R` is constructed from its inner value
                let prev_r_inner = subgroup_shuffle(
                    if lane_id == subgroup_size - 1 {
                        prev_iter_r_inner
                    } else {
                        r_inner
                    },
                    lane_id.wrapping_sub(1) % subgroup_size,
                );
                // SAFETY: `R` is constructed from its inner value
                let before_prev_r = subgroup_shuffle(
                    if lane_id >= subgroup_size - 2 {
                        prev_iter_r_inner
                    } else {
                        r_inner
                    },
                    lane_id.wrapping_sub(2) % subgroup_size,
                );

                let is_first = !ignore && r_inner != prev_r_inner;
                let is_second = !ignore && r_inner == prev_r_inner && r_inner != before_prev_r;

                let positions_offset =
                    positions_cursor + subgroup_exclusive_i_add(if is_first { 1 } else { 0 });
                let prev_positions_offset =
                    subgroup_shuffle(positions_offset, lane_id.wrapping_sub(1));

                let data = if is_first {
                    ConcurrentAddSlot::First {
                        physical_pointer: positions_offset,
                    }
                    .into_data()
                } else if is_second {
                    ConcurrentAddSlot::Second {
                        physical_pointer: if lane_id == 0 {
                            positions_cursor - 1
                        } else {
                            prev_positions_offset
                        },
                    }
                    .into_data()
                } else {
                    ConcurrentAddSlot::Ignore.into_data()
                };

                positions_cursor = subgroup_u_max(positions_offset + if is_first { 1 } else { 0 });

                // SAFETY: `r_inner` is valid according to the function contract, `data` part is
                // statically known to fit
                shared_bucket[local_offset].r = unsafe { R::new_with_data(r_inner, data) };
                prev_iter_r_inner = r_inner;

                // TODO: `subgroup_shuffle` on bool would have been nice, but it is not supported by
                //  wgpu
                if subgroup_shuffle(ignore as u32, subgroup_size - 1) == 1 {
                    break;
                }
            }
        } else {
            assert_eq!(lane_id, 0);
            assert_eq!(subgroup_size, 1);

            // SAFETY: `R` inner value is valid according to the function contract, `data` part is
            // statically known to fit
            shared_bucket[0].r = unsafe {
                R::new_with_data(shared_bucket[0].r.get_inner(), {
                    // The very first one is never ignored
                    let data = ConcurrentAddSlot::First {
                        physical_pointer: positions_cursor,
                    }
                    .into_data();
                    positions_cursor += 1;
                    data
                })
            };

            for local_offset in 1..REDUCED_BUCKET_SIZE {
                let position = shared_bucket[local_offset].position;
                if position == Position::SENTINEL {
                    break;
                }
                let r = shared_bucket[local_offset].r.get_inner();

                let (prev_r, prev_data) = shared_bucket[local_offset - 1].r.split();

                let data = if r == prev_r {
                    if ConcurrentAddSlot::is_first(prev_data) {
                        // SAFETY: Obtained from `ConcurrentAddSlot` on the previous iteration
                        unsafe { ConcurrentAddSlot::data_to_second(prev_data) }
                    } else {
                        ConcurrentAddSlot::Ignore.into_data()
                    }
                } else {
                    let data = ConcurrentAddSlot::First {
                        physical_pointer: positions_cursor,
                    }
                    .into_data();
                    positions_cursor += 1;
                    data
                };

                // SAFETY: `r_inner` is valid according to the function contract, `data` part is
                // statically known to fit
                shared_bucket[local_offset].r = unsafe { R::new_with_data(r, data) };
            }
        }
    }
}
