#[cfg(all(test, not(target_arch = "spirv")))]
mod tests;

use crate::shader::constants::{PARAM_BC, REDUCED_BUCKET_SIZE};
#[cfg(target_arch = "spirv")]
use crate::shader::find_matches_in_buckets::ArrayIndexingPolyfill;
use crate::shader::types::{Position, PositionExt, PositionR, R};
use core::mem::MaybeUninit;
#[cfg(target_arch = "spirv")]
use spirv_std::arch::{atomic_or, subgroup_shuffle};
#[cfg(target_arch = "spirv")]
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

pub(super) type RmapBitPosition = u32;

// TODO: Remove once normal `RmapBitPosition` struct can be used
pub(super) trait RmapBitPositionExt: Sized {
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

/// Preparation state that combines a bunch of separate tracking values into a compact
/// representation to minimize registers usage to maintain reasonable occupancy
#[derive(Debug)]
struct RAccumulator {
    /// Accumulates `r`, virtual pointer, and a flag whether `r` duplicate was found
    r_accumulator: u32,
}

impl Default for RAccumulator {
    #[inline(always)]
    fn default() -> Self {
        Self {
            // TODO: `const {}` is a workaround for https://github.com/Rust-GPU/rust-gpu/issues/322 and
            //  shouldn't be necessary otherwise
            r_accumulator: const { u32::MAX >> (u32::BITS - (PARAM_BC - 1).bit_width()) },
        }
    }
}

impl RAccumulator {
    #[inline(always)]
    fn get_additional_data(&self) -> u32 {
        self.r_accumulator >> (u32::BITS - (VIRTUAL_POINTER_BITS + 1))
    }

    #[inline(always)]
    fn set_r_duplicate(&mut self) {
        let r_duplicate = 1 << (u32::BITS - (VIRTUAL_POINTER_BITS + 1));
        self.r_accumulator |= r_duplicate;
    }

    #[inline(always)]
    fn has_r_duplicate(&self) -> bool {
        let r_duplicate = 1 << (u32::BITS - (VIRTUAL_POINTER_BITS + 1));
        (self.r_accumulator & r_duplicate) != 0
    }

    #[inline(always)]
    fn set_r(&mut self, r: u32) {
        // Statically ensure all 3 components fit into `u32`
        const {
            assert!(VIRTUAL_POINTER_BITS + 1 + (PARAM_BC - 1).bit_width() <= u32::BITS);
        }
        // Clear everything except the virtual pointer
        self.r_accumulator &= u32::MAX << (u32::BITS - VIRTUAL_POINTER_BITS);
        // Increment virtual pointer
        let virtual_pointer_increment = 1 << (u32::BITS - VIRTUAL_POINTER_BITS);
        self.r_accumulator += virtual_pointer_increment;
        // Set `r`
        self.r_accumulator |= r;
    }

    #[inline(always)]
    fn get_r(&self) -> u32 {
        // TODO: `const {}` is a workaround for https://github.com/Rust-GPU/rust-gpu/issues/322 and
        //  shouldn't be necessary otherwise
        self.r_accumulator & const { u32::MAX >> (u32::BITS - (PARAM_BC - 1).bit_width()) }
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
            positions: [[Position::ZERO; 2]; _],
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
        if rmap_item[0] == Position::ZERO {
            rmap_item[0] = position;
        } else if rmap_item[1] == Position::ZERO {
            rmap_item[1] = position;
        }
    }

    /// Add using `r` that was previously updated using [`Self::update_local_bucket_r_data()`] in a
    /// fully parallel way (all threads can contribute at once).
    ///
    /// # Safety
    /// `r` elements must be updated using [`Self::update_local_bucket_r_data()`].
    pub(super) unsafe fn add_with_data_parallel(&mut self, r: R, position: Position) {
        let (r, data) = r.split();
        if data == 0 {
            // No virtual pointer here, hence this is a duplicate that should be ignored
            return;
        }
        let rmap_offset = data & 1;
        let virtual_pointer = data >> 1;

        // SAFETY: `r` is obtained from the `R` instance and thus must be valid
        let rmap_bit_position = unsafe { RmapBitPosition::new(r) };

        let bit_position = rmap_bit_position.get();
        let word_offset = (bit_position / u32::BITS) as usize;
        let bit_offset = bit_position % u32::BITS;

        // SAFETY: Offset comes from `RmapBitPosition`, whose constructor guarantees bounds
        let word = unsafe { self.virtual_pointers.get_unchecked_mut(word_offset) };
        // TODO: Probably should not be unsafe to begin with:
        //  https://github.com/Rust-GPU/rust-gpu/pull/394#issuecomment-3316594485
        #[cfg(target_arch = "spirv")]
        unsafe {
            atomic_or::<_, { Scope::Workgroup as u32 }, { Semantics::NONE.bits() }>(
                word,
                virtual_pointer << bit_offset,
            );
        }
        #[cfg(not(target_arch = "spirv"))]
        {
            *word |= virtual_pointer << bit_offset;
        }

        if bit_offset + VIRTUAL_POINTER_BITS > u32::BITS {
            // SAFETY: Offset comes from `RmapBitPosition`, whose constructor guarantees bounds
            let word_next = unsafe { self.virtual_pointers.get_unchecked_mut(word_offset + 1) };
            // TODO: Probably should not be unsafe to begin with:
            //  https://github.com/Rust-GPU/rust-gpu/pull/394#issuecomment-3316594485
            #[cfg(target_arch = "spirv")]
            unsafe {
                atomic_or::<_, { Scope::Workgroup as u32 }, { Semantics::NONE.bits() }>(
                    word_next,
                    virtual_pointer >> (u32::BITS - bit_offset),
                );
            }
            #[cfg(not(target_arch = "spirv"))]
            {
                *word_next |= virtual_pointer >> (u32::BITS - bit_offset);
            }
        }

        let physical_pointer = virtual_pointer - 1;

        // SAFETY: Internal pointers are always valid
        let rmap_item = unsafe { self.positions.get_unchecked_mut(physical_pointer as usize) };
        rmap_item[rmap_offset as usize] = position;
    }

    #[inline(always)]
    pub(super) fn get(&self, rmap_bit_position: RmapBitPosition) -> [Position; 2] {
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
            [Position::ZERO; 2]
        }
    }

    // TODO: Remove as soon as non-hacky version compiles
    #[inline(always)]
    pub(super) fn zeroing_hack(
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
            rmap.positions[bucket as usize][pair_offset as usize] = Position::ZERO;
        }
    }

    // TODO: It should be possible to optimize this further with parallelism instead of (currently)
    //  sequential version
    /// This prepares the local bucket by appending extra information in `R`'s data field, such that
    /// `Rmap` can later be constructed in parallel rather than sequentially.
    ///
    /// Each `local_bucket` stores its slice of elements of the whole bucket.
    ///
    /// NOTE: For this to work correctly, all local buckets together must be sorted by `r` and
    /// `position` among `r` duplicates. `r` must not store additional data in it yet.
    ///
    /// /// # Safety
    /// There must be at most [`REDUCED_BUCKET_SIZE`] items inserted.
    pub(in super::super) unsafe fn update_local_bucket_r_data<const ELEMENTS_PER_THREAD: usize>(
        lane_id: u32,
        subgroup_size: u32,
        local_bucket: &mut [PositionR; ELEMENTS_PER_THREAD],
    ) {
        let mut preparation_state = RAccumulator::default();

        // TODO: More idiomatic version currently doesn't compile:
        //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
        #[expect(
            clippy::needless_range_loop,
            reason = "rust-gpu can't compile idiomatic version"
        )]
        for source_lane in 0..subgroup_size {
            for local_offset in 0..ELEMENTS_PER_THREAD {
                #[cfg(target_arch = "spirv")]
                let position = subgroup_shuffle(local_bucket[local_offset].position, source_lane);
                #[cfg(not(target_arch = "spirv"))]
                let position = {
                    assert_eq!(lane_id, 0);
                    assert_eq!(subgroup_size, 1);

                    local_bucket[local_offset].position
                };
                if position == Position::ZERO {
                    // This is to match the sequential version
                    continue;
                }
                if position == Position::SENTINEL {
                    return;
                }

                #[cfg(target_arch = "spirv")]
                let r = subgroup_shuffle(local_bucket[local_offset].r.get_inner(), source_lane);
                #[cfg(not(target_arch = "spirv"))]
                let r = {
                    assert_eq!(lane_id, 0);
                    assert_eq!(subgroup_size, 1);

                    local_bucket[local_offset].r.get_inner()
                };

                if preparation_state.get_r() == r {
                    if preparation_state.has_r_duplicate() {
                        // One `r` duplicate was already processed, all others are skipped
                        continue;
                    }
                    preparation_state.set_r_duplicate();
                } else {
                    preparation_state.set_r(r);
                }

                if lane_id == source_lane {
                    #[expect(clippy::int_plus_one, reason = "This describes the invariant exactly")]
                    const {
                        assert!(VIRTUAL_POINTER_BITS + 1 <= u32::BITS - (PARAM_BC - 1).bit_width());
                    }
                    // `r` is valid according to the function contract, `data` part is statically
                    // asserted to fit above
                    local_bucket[local_offset].r =
                        unsafe { R::new_with_data(r, preparation_state.get_additional_data()) };

                    #[cfg(not(target_arch = "spirv"))]
                    {
                        assert_eq!(r, local_bucket[local_offset].r.split().0);
                        assert_eq!(
                            preparation_state.get_additional_data(),
                            local_bucket[local_offset].r.split().1
                        );
                    }
                }
            }
        }
    }
}
