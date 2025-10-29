use crate::shader::constants::{
    MAX_BUCKET_SIZE, PARAM_BC, PARAM_M, REDUCED_BUCKET_SIZE, REDUCED_MATCHES_COUNT,
};
use crate::shader::find_matches_in_buckets::{Match, calculate_left_target_on_demand};
use crate::shader::types::{Position, PositionExt, PositionR, Y};
use std::mem::MaybeUninit;

struct Rmap {
    /// `0` is a sentinel value indicating no virtual pointer is stored yet.
    ///
    /// Physical pointer must be increased by `1` to get a virtual pointer before storing. Virtual
    /// pointer must be decreased by `1` before reading to get a physical pointer.
    virtual_pointers: [u16; PARAM_BC as usize],
    positions: [[Position; 2]; REDUCED_BUCKET_SIZE],
    next_physical_pointer: u16,
}

impl Rmap {
    #[inline(always)]
    fn new() -> Self {
        Self {
            virtual_pointers: [0; _],
            positions: [[Position::ZERO; 2]; _],
            next_physical_pointer: 0,
        }
    }

    /// # Safety
    /// `r` must be in the range `0..PARAM_BC`, there must be at most [`REDUCED_BUCKET_SIZE`] items
    /// inserted
    #[inline(always)]
    unsafe fn insertion_item(&mut self, r: u32) -> &mut [Position; 2] {
        // SAFETY: Guaranteed by function contract
        let virtual_pointer = unsafe { self.virtual_pointers.get_unchecked_mut(r as usize) };

        if let Some(physical_pointer) = virtual_pointer.checked_sub(1) {
            // SAFETY: Internal pointers are always valid
            return unsafe { self.positions.get_unchecked_mut(physical_pointer as usize) };
        }

        let physical_pointer = self.next_physical_pointer;
        self.next_physical_pointer += 1;
        *virtual_pointer = physical_pointer + 1;

        // SAFETY: It is guaranteed by the function contract that the number of added elements will
        // never exceed `REDUCED_BUCKETS_SIZE`, hence allocated pointers will always be within
        // bounds
        unsafe { self.positions.get_unchecked_mut(physical_pointer as usize) }
    }

    /// Note that `position == Position::ZERO` is effectively ignored here, supporting it cost too
    /// much in terms of performance and not required for correctness.
    ///
    /// # Safety
    /// `r` must be in the range `0..PARAM_BC`, there must be at most [`REDUCED_BUCKET_SIZE`] items
    /// inserted
    #[inline(always)]
    unsafe fn add(&mut self, r: u32, position: Position) {
        // SAFETY: Guaranteed by function contract
        let rmap_item = unsafe { self.insertion_item(r) };

        // The same `r` can appear in the table multiple times, one duplicate is supported here
        if rmap_item[0] == Position::ZERO {
            rmap_item[0] = position;
        } else if rmap_item[1] == Position::ZERO {
            rmap_item[1] = position;
        }
    }

    /// # Safety
    /// `r` must be in the range `0..PARAM_BC`
    #[inline(always)]
    unsafe fn get(&self, r: u32) -> [Position; 2] {
        // SAFETY: Guaranteed by function contract
        let virtual_pointer = *unsafe { self.virtual_pointers.get_unchecked(r as usize) };

        if let Some(physical_pointer) = virtual_pointer.checked_sub(1) {
            // SAFETY: Internal pointers are always valid
            *unsafe { self.positions.get_unchecked(physical_pointer as usize) }
        } else {
            [Position::ZERO; 2]
        }
    }
}

/// For verification use [`has_match`] instead.
pub(in super::super) fn find_matches_in_buckets_correct<'a>(
    left_bucket_index: u32,
    left_bucket: &[PositionR; MAX_BUCKET_SIZE],
    right_bucket: &[PositionR; MAX_BUCKET_SIZE],
    // `PARAM_M as usize * 2` corresponds to the upper bound number of matches a single `y` in the
    // left bucket might have here
    matches: &'a mut [MaybeUninit<Match>; REDUCED_MATCHES_COUNT + PARAM_M as usize * 2],
) -> &'a [Match] {
    let left_base = left_bucket_index * u32::from(PARAM_BC);

    let mut rmap = Rmap::new();
    for &PositionR { position, r } in right_bucket {
        if position == Position::SENTINEL {
            break;
        }
        let (r, _data) = r.split();
        // SAFETY: `r` is within `0..PARAM_BC` range by definition, the right bucket is limited to
        // `REDUCED_BUCKETS_SIZE`
        unsafe {
            rmap.add(r, position);
        }
    }

    let parity = left_base % 2;
    let mut next_match_index = 0;

    // TODO: Simd read for left bucket? It might be more efficient in terms of memory access to
    //  process chunks of the left bucket against one right value for each at a time
    for &PositionR { position, r } in left_bucket {
        // `next_match_index >= REDUCED_MATCHES_COUNT` is crucial to make sure
        if position == Position::SENTINEL || next_match_index >= REDUCED_MATCHES_COUNT {
            // Sentinel values are padded to the end of the bucket
            break;
        }

        let (r, _data) = r.split();

        for m in 0..u32::from(PARAM_M) {
            let r_target = calculate_left_target_on_demand(parity, r, m);
            // SAFETY: Targets are always limited to `PARAM_BC`
            let [right_position_a, right_position_b] = unsafe { rmap.get(r_target) };

            // The right bucket position is never zero
            if right_position_a != Position::ZERO {
                let left_y = Y::from(r + left_base);
                // SAFETY: Iteration will stop before `REDUCED_MATCHES_COUNT + PARAM_M * 2`
                // elements is inserted
                unsafe { matches.get_unchecked_mut(next_match_index) }.write(Match {
                    left_position: position,
                    left_y,
                    right_position: right_position_a,
                });
                next_match_index += 1;

                if right_position_b != Position::ZERO {
                    // SAFETY: Iteration will stop before
                    // `REDUCED_MATCHES_COUNT + PARAM_M * 2` elements is inserted
                    unsafe { matches.get_unchecked_mut(next_match_index) }.write(Match {
                        left_position: position,
                        left_y,
                        right_position: right_position_b,
                    });
                    next_match_index += 1;
                }
            }
        }
    }

    // SAFETY: Initialized this many matches, limited to `REDUCED_MATCHES_COUNT`
    unsafe { matches[..next_match_index.min(REDUCED_MATCHES_COUNT)].assume_init_ref() }
}
