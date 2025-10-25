#[cfg(all(test, not(target_arch = "spirv")))]
mod tests;

use crate::shader::constants::{PARAM_BC, REDUCED_BUCKET_SIZE};
#[cfg(target_arch = "spirv")]
use crate::shader::find_matches_in_buckets::ArrayIndexingPolyfill;
use crate::shader::types::{Position, PositionExt};
use core::mem::MaybeUninit;

// TODO: Benchmark on different GPUs to see if the complexity of dealing with 9-bit pointers is
//  worth it or maybe using u16s would be better despite using more shared memory
/// Number of bits necessary to address a single pair of positions in the rmap
const POINTER_BITS: u32 = REDUCED_BUCKET_SIZE.bit_width();
const POINTERS_BITS: usize = PARAM_BC as usize * POINTER_BITS as usize;
const POINTERS_WORDS: usize = POINTERS_BITS.div_ceil(u32::BITS as usize);

// Ensure `u32` is sufficiently large as a container
const _: () = assert!(POINTER_BITS <= u32::BITS);

#[derive(Debug, Default)]
pub(super) struct NextPhysicalPointer {
    next_physical_pointer: u32,
}

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
//         Self(r * POINTER_BITS)
//     }
//
//     /// Extract `rmap_bit_position` out of the inner value
//     #[inline(always)]
//     fn get(self) -> u32 {
//         self.0
//     }
//
//     #[inline(always)]
//     pub(super) const fn uninit_array_from_repr_mut<const N: usize>(
//         array: &mut [MaybeUninit<u32>; N],
//     ) -> &mut [MaybeUninit<Self>; N] {
//         // SAFETY: `RmapBitPosition` is `#[repr(C)]` and guaranteed to have the same memory layout
//         unsafe { mem::transmute(array) }
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

    fn uninit_array_from_repr_mut<const N: usize>(
        array: &mut [MaybeUninit<u32>; N],
    ) -> &mut [MaybeUninit<Self>; N];
}

impl RmapBitPositionExt for RmapBitPosition {
    /// # Safety
    /// `r` must be in the range `0..PARAM_BC`
    #[inline(always)]
    unsafe fn new(r: u32) -> Self {
        r * POINTER_BITS
    }

    /// Extract `rmap_bit_position` out of the inner value
    #[inline(always)]
    fn get(self) -> u32 {
        self
    }

    #[inline(always)]
    fn uninit_array_from_repr_mut<const N: usize>(
        array: &mut [MaybeUninit<u32>; N],
    ) -> &mut [MaybeUninit<Self>; N] {
        array
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
    fn new() -> Self {
        Self {
            virtual_pointers: [0; _],
            positions: [[Position::ZERO; 2]; _],
        }
    }

    /// # Safety
    /// There must be at most [`REDUCED_BUCKET_SIZE`] items inserted. `NextPhysicalPointer` and
    /// `Rmap` must have 1:1 mapping and not mixed with anything else.
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

        if bit_offset + POINTER_BITS > u32::BITS {
            // SAFETY: Offset comes from `RmapBitPosition`, whose constructor guarantees bounds
            let mut word_next =
                *unsafe { self.virtual_pointers.get_unchecked_mut(word_offset + 1) };
            {
                let value = (word >> bit_offset) | (word_next << (u32::BITS - bit_offset));
                let virtual_pointer = value & (u32::MAX >> (u32::BITS - POINTER_BITS));

                if let Some(physical_pointer) = virtual_pointer.checked_sub(1) {
                    return physical_pointer;
                }
            }

            let physical_pointer = next_physical_pointer.inc();
            let virtual_pointer = physical_pointer + 1;

            word |= virtual_pointer << bit_offset;
            word_next |= virtual_pointer >> (u32::BITS - bit_offset);

            *unsafe { self.virtual_pointers.get_unchecked_mut(word_offset) } = word;
            *unsafe { self.virtual_pointers.get_unchecked_mut(word_offset + 1) } = word_next;

            physical_pointer
        } else {
            {
                let virtual_pointer =
                    (word >> bit_offset) & (u32::MAX >> (u32::BITS - POINTER_BITS));

                if let Some(physical_pointer) = virtual_pointer.checked_sub(1) {
                    return physical_pointer;
                }
            }

            let physical_pointer = next_physical_pointer.inc();
            let virtual_pointer = physical_pointer + 1;

            word |= virtual_pointer << bit_offset;

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

    #[inline(always)]
    pub(super) fn get(&self, rmap_bit_position: RmapBitPosition) -> [Position; 2] {
        let bit_position = rmap_bit_position.get();
        let word_offset = (bit_position / u32::BITS) as usize;
        let bit_offset = bit_position % u32::BITS;

        let virtual_pointer = if bit_offset + POINTER_BITS > u32::BITS {
            // SAFETY: Offset comes from `RmapBitPosition`, whose constructor guarantees bounds
            let word = unsafe { *self.virtual_pointers.get_unchecked(word_offset) };
            // SAFETY: Offset comes from `RmapBitPosition`, whose constructor guarantees bounds
            let word_next = unsafe { *self.virtual_pointers.get_unchecked(word_offset + 1) };

            let value = (word >> bit_offset) | (word_next << (u32::BITS - bit_offset));
            value & (u32::MAX >> (u32::BITS - POINTER_BITS))
        } else {
            // SAFETY: Offset comes from `RmapBitPosition`, whose constructor guarantees bounds
            let word = unsafe { *self.virtual_pointers.get_unchecked(word_offset) };

            (word >> bit_offset) & (u32::MAX >> (u32::BITS - POINTER_BITS))
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
}
