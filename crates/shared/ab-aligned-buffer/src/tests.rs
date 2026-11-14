use crate::alignment::{Align1, Align16, Align32, Align4096, Alignment, AtLeast};
use crate::{AlignedBuffer, EMPTY_ALLOCATION, OwnedAlignedBuffer, SharedAlignedBuffer};
use alloc::vec;
use alloc::vec::Vec;
use core::iter;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::Ordering;

const EXPECTED_ALIGNMENT: usize = 16;

#[test]
#[expect(
    clippy::explicit_deref_methods,
    reason = "Intentionally explicit for testing"
)]
fn basic() {
    for capacity in 0..=EXPECTED_ALIGNMENT as u32 {
        let mut owned = OwnedAlignedBuffer::<Align16>::with_capacity(capacity);
        assert_eq!(owned.len(), 0, "Capacity {capacity}");
        assert!(owned.capacity() >= capacity, "Capacity {capacity}");
        assert!(owned.is_empty(), "Capacity {capacity}");
        assert_eq!(owned.as_slice(), owned.deref(), "Capacity {capacity}");
        assert_eq!(
            owned.as_mut_slice().as_mut_ptr(),
            owned.deref_mut().as_mut_ptr(),
            "Capacity {capacity}"
        );
        assert!(owned.as_slice().is_empty(), "Capacity {capacity}");
        assert!(owned.as_mut_slice().is_empty(), "Capacity {capacity}");
        assert_eq!(owned.as_ptr(), owned.as_mut_ptr(), "Capacity {capacity}");
        assert!(
            owned.as_ptr().is_aligned_to(EXPECTED_ALIGNMENT),
            "Capacity {capacity}"
        );

        let ptr_before = owned.as_ptr();

        // Using part of the capacity
        {
            let len = owned.capacity().saturating_sub(1);
            let bytes = vec![1; len as usize];
            owned.copy_from_slice(&bytes);
            assert_eq!(owned.len(), len, "Capacity {capacity}");
            assert_eq!(owned.as_slice(), owned.deref(), "Capacity {capacity}");
            assert_eq!(
                owned.as_mut_slice().as_mut_ptr(),
                owned.deref_mut().as_mut_ptr(),
                "Capacity {capacity}"
            );
            assert_eq!(owned.as_slice().len(), len as usize, "Capacity {capacity}");
            assert_eq!(
                owned.as_mut_slice().len(),
                len as usize,
                "Capacity {capacity}"
            );
            assert!(owned.capacity() >= capacity, "Capacity {capacity}");
            assert_eq!(owned.is_empty(), len == 0, "Capacity {capacity}");
            assert_eq!(owned.as_ptr(), owned.as_mut_ptr(), "Capacity {capacity}");
            assert_eq!(owned.as_ptr(), ptr_before, "Capacity {capacity}");
            assert!(
                owned.as_ptr().is_aligned_to(EXPECTED_ALIGNMENT),
                "Capacity {capacity}"
            );

            let mut owned2 = OwnedAlignedBuffer::<Align16>::from_bytes(&bytes);
            let shared = SharedAlignedBuffer::<Align16>::from_bytes(&bytes);
            assert_eq!(owned.len(), owned2.len(), "Capacity {capacity}");
            assert_eq!(owned.len(), shared.len(), "Capacity {capacity}");
            assert_eq!(owned.is_empty(), owned2.is_empty(), "Capacity {capacity}");
            assert_eq!(owned.is_empty(), shared.is_empty(), "Capacity {capacity}");
            assert_eq!(owned2.as_slice(), owned2.deref(), "Capacity {capacity}");
            assert_eq!(
                owned2.as_mut_slice().as_mut_ptr(),
                owned2.deref_mut().as_mut_ptr(),
                "Capacity {capacity}"
            );
            assert_eq!(owned.as_slice(), owned2.as_slice(), "Capacity {capacity}");
            assert_eq!(owned.as_slice(), shared.as_slice(), "Capacity {capacity}");
            assert_eq!(
                owned.as_mut_slice(),
                owned2.as_mut_slice(),
                "Capacity {capacity}"
            );
        }

        // Using full capacity
        {
            let len = owned.capacity();
            let bytes = vec![1; len as usize];
            owned.copy_from_slice(&bytes);
            assert_eq!(owned.len(), len, "Capacity {capacity}");
            assert_eq!(owned.as_slice(), owned.deref(), "Capacity {capacity}");
            assert_eq!(
                owned.as_mut_slice().as_mut_ptr(),
                owned.deref_mut().as_mut_ptr(),
                "Capacity {capacity}"
            );
            assert_eq!(owned.as_slice().len(), len as usize, "Capacity {capacity}");
            assert_eq!(
                owned.as_mut_slice().len(),
                len as usize,
                "Capacity {capacity}"
            );
            assert!(owned.capacity() >= capacity, "Capacity {capacity}");
            assert_eq!(owned.is_empty(), len == 0, "Capacity {capacity}");
            assert_eq!(owned.as_ptr(), owned.as_mut_ptr(), "Capacity {capacity}");
            assert_eq!(owned.as_ptr(), ptr_before, "Capacity {capacity}");
            assert!(
                owned.as_ptr().is_aligned_to(EXPECTED_ALIGNMENT),
                "Capacity {capacity}"
            );

            let mut owned2 = OwnedAlignedBuffer::<Align16>::from_bytes(&bytes);
            let shared = SharedAlignedBuffer::<Align16>::from_bytes(&bytes);
            assert_eq!(owned.len(), owned2.len(), "Capacity {capacity}");
            assert_eq!(owned.len(), shared.len(), "Capacity {capacity}");
            assert_eq!(owned.is_empty(), owned2.is_empty(), "Capacity {capacity}");
            assert_eq!(owned.is_empty(), shared.is_empty(), "Capacity {capacity}");
            assert_eq!(owned2.as_slice(), owned2.deref(), "Capacity {capacity}");
            assert_eq!(
                owned2.as_mut_slice().as_mut_ptr(),
                owned2.deref_mut().as_mut_ptr(),
                "Capacity {capacity}"
            );
            assert_eq!(owned.as_slice(), owned2.as_slice(), "Capacity {capacity}");
            assert_eq!(owned.as_slice(), shared.as_slice(), "Capacity {capacity}");
            assert_eq!(
                owned.as_mut_slice(),
                owned2.as_mut_slice(),
                "Capacity {capacity}"
            );
        }

        // Exceed capacity, resulting in reallocation
        {
            let len = owned.capacity() + 1;
            let bytes = vec![1; len as usize];
            owned.copy_from_slice(&bytes);
            assert_eq!(owned.len(), len, "Capacity {capacity}");
            assert_eq!(owned.as_slice(), owned.deref(), "Capacity {capacity}");
            assert_eq!(
                owned.as_mut_slice().as_mut_ptr(),
                owned.deref_mut().as_mut_ptr(),
                "Capacity {capacity}"
            );
            assert_eq!(owned.as_slice().len(), len as usize, "Capacity {capacity}");
            assert_eq!(
                owned.as_mut_slice().len(),
                len as usize,
                "Capacity {capacity}"
            );
            assert!(owned.capacity() >= capacity, "Capacity {capacity}");
            assert!(!owned.is_empty(), "Capacity {capacity}");
            assert_eq!(owned.as_ptr(), owned.as_mut_ptr(), "Capacity {capacity}");
            assert!(
                owned.as_ptr().is_aligned_to(EXPECTED_ALIGNMENT),
                "Capacity {capacity}"
            );

            let mut owned2 = OwnedAlignedBuffer::<Align16>::from_bytes(&bytes);
            let shared = SharedAlignedBuffer::<Align16>::from_bytes(&bytes);
            assert_eq!(owned.len(), owned2.len(), "Capacity {capacity}");
            assert_eq!(owned.len(), shared.len(), "Capacity {capacity}");
            assert_eq!(owned.is_empty(), owned2.is_empty(), "Capacity {capacity}");
            assert_eq!(owned.is_empty(), shared.is_empty(), "Capacity {capacity}");
            assert_eq!(owned2.as_slice(), owned2.deref(), "Capacity {capacity}");
            assert_eq!(
                owned2.as_mut_slice().as_mut_ptr(),
                owned2.deref_mut().as_mut_ptr(),
                "Capacity {capacity}"
            );
            assert_eq!(owned.as_slice(), owned2.as_slice(), "Capacity {capacity}");
            assert_eq!(owned.as_slice(), shared.as_slice(), "Capacity {capacity}");
            assert_eq!(
                owned.as_mut_slice(),
                owned2.as_mut_slice(),
                "Capacity {capacity}"
            );

            let shorter_len = owned2.len() - 1;
            // SAFETY: length is guaranteed to be within stored bytes
            unsafe {
                owned2.set_len(shorter_len);
            }
            assert_eq!(&owned.as_slice()[..shorter_len as usize], owned2.as_slice());
        }

        // Create a shared instance
        let owned_capacity = owned.capacity();
        let shared = owned.into_shared();
        assert_eq!(shared.as_slice(), shared.deref(), "Capacity {capacity}");
        assert_eq!(shared.capacity(), owned_capacity, "Capacity {capacity}");
        let ptr_before = shared.as_ptr();
        // Turn back into owned and confirm that it points to the same memory (meaning no additional
        // allocation)
        let owned = shared.into_owned();
        assert_eq!(owned.as_ptr(), ptr_before, "Capacity {capacity}");

        let shared = owned.into_shared();
        assert_eq!(shared.as_slice(), shared.deref(), "Capacity {capacity}");
        // Cloned shared instance will result in new allocation
        let owned = shared.clone().into_owned();
        assert_ne!(owned.as_ptr(), ptr_before, "Capacity {capacity}");

        let shared2 = shared.clone();
        assert_eq!(shared2.as_slice(), shared2.deref(), "Capacity {capacity}");
        assert_eq!(shared.as_slice(), shared2.as_slice(), "Capacity {capacity}");
        assert_eq!(owned.as_slice(), shared.as_slice(), "Capacity {capacity}");

        assert_eq!(shared.len(), shared2.len(), "Capacity {capacity}");
        assert_eq!(shared.len(), owned.len(), "Capacity {capacity}");
        assert_eq!(shared.is_empty(), shared2.is_empty(), "Capacity {capacity}");
        assert_eq!(shared.is_empty(), owned.is_empty(), "Capacity {capacity}");
        assert_eq!(shared.as_ptr(), shared2.as_ptr(), "Capacity {capacity}");
        assert_eq!(shared.as_slice(), shared2.as_slice(), "Capacity {capacity}");
    }
}

#[test]
fn realloc() {
    let mut owned = OwnedAlignedBuffer::<Align16>::with_capacity(10);
    assert!(owned.append(b"abc"));

    let original_capacity = owned.capacity();
    assert!(original_capacity >= 10);

    owned.ensure_capacity(1000);
    assert_ne!(original_capacity, owned.capacity());
    assert!(owned.capacity() >= 1000);
    assert_eq!(owned.as_slice(), b"abc");

    let ptr_before_append = owned.as_ptr();
    assert!(owned.append(&[1; 1000 - 3]));
    // Ensure it didn't reallocate
    assert_eq!(ptr_before_append, owned.as_ptr());
}

#[test]
fn as_u128_slice() {
    let bytes = (0..=u8::MAX).collect::<Vec<_>>();

    for len in 0..=3 * size_of::<u128>() {
        let owned = OwnedAlignedBuffer::<Align16>::from_bytes(&bytes[..len]);
        let u128s = owned.as_u128_slice();
        assert_eq!(u128s.as_ptr(), owned.as_ptr(), "Length {len}");
        // Only whole `u128`s are included
        assert_eq!(u128s.len(), len / size_of::<u128>(), "Length {len}");
        assert_eq!(
            u128s
                .iter()
                .flat_map(|value| value.to_ne_bytes())
                .collect::<Vec<_>>(),
            &bytes[..size_of_val(u128s)],
            "Length {len}"
        );
    }
}

#[test]
fn resize_through_unaligned_capacities() {
    let bytes = (1..=u8::MAX).collect::<Vec<_>>();
    let mut owned = OwnedAlignedBuffer::<Align16>::with_capacity(1);
    assert!(owned.append(&bytes[..1]));

    // Every capacity that is not a multiple of the alignment, and those that are
    for capacity in 1..=40 {
        owned.ensure_capacity(capacity);
        assert!(owned.capacity() >= capacity, "Capacity {capacity}");
        assert!(
            owned.as_ptr().is_aligned_to(EXPECTED_ALIGNMENT),
            "Capacity {capacity}"
        );
        assert_eq!(owned.as_slice(), &bytes[..1], "Capacity {capacity}");
    }

    // Shorter and longer contents, the last one grows the buffer again
    for len in [3, 17, 33, 3, 17, 100] {
        owned.copy_from_slice(&bytes[..len]);
        assert!(
            owned.as_ptr().is_aligned_to(EXPECTED_ALIGNMENT),
            "Length {len}"
        );
        assert_eq!(owned.as_slice(), &bytes[..len], "Length {len}");
    }

    let shared = owned.into_shared();
    let shared2 = shared.clone();
    drop(shared);
    assert!(shared2.as_ptr().is_aligned_to(EXPECTED_ALIGNMENT));
    assert_eq!(shared2.as_slice(), &bytes[..100]);
}

#[test]
fn strong_count_survives_clones() {
    let empty = SharedAlignedBuffer::<Align16>::default();
    let shared = SharedAlignedBuffer::<Align16>::from_bytes(b"abc");
    let ptr_before = shared.as_ptr();

    let clones = iter::repeat_with(|| (empty.clone(), shared.clone()))
        .take(1000)
        .collect::<Vec<_>>();
    for (empty_clone, shared_clone) in &clones {
        assert!(empty_clone.is_empty());
        assert_eq!(shared_clone.as_ptr(), ptr_before);
    }
    drop(clones);

    // The static empty buffer is still alive
    drop(empty);
    assert!(SharedAlignedBuffer::<Align16>::empty_ref().is_empty());
    assert!(
        SharedAlignedBuffer::<Align16>::default()
            .into_owned()
            .is_empty()
    );

    // The last instance reuses its allocation
    let owned = shared.into_owned();
    assert_eq!(owned.as_ptr(), ptr_before);
    assert_eq!(owned.as_slice(), b"abc");
}

#[test]
fn empty_buffers_are_not_backed_by_allocation() {
    let empty_ptr = SharedAlignedBuffer::<Align16>::default().as_ptr();

    assert_eq!(
        SharedAlignedBuffer::<Align16>::empty_ref().as_ptr(),
        empty_ptr
    );
    assert_eq!(
        SharedAlignedBuffer::<Align16>::from_bytes(&[]).as_ptr(),
        empty_ptr
    );
    assert_eq!(
        OwnedAlignedBuffer::<Align16>::with_capacity(0).as_ptr(),
        empty_ptr
    );
    assert_eq!(
        OwnedAlignedBuffer::<Align16>::from_bytes(&[]).as_ptr(),
        empty_ptr
    );
    assert_eq!(
        OwnedAlignedBuffer::<Align16>::with_capacity(0).capacity(),
        0
    );
    assert_eq!(SharedAlignedBuffer::<Align16>::default().capacity(), 0);
    assert!(
        OwnedAlignedBuffer::<Align16>::with_capacity(0)
            .as_ptr()
            .is_aligned_to(EXPECTED_ALIGNMENT)
    );

    // Empty shared buffer doesn't keep the allocation of an owned buffer with capacity
    let shared = OwnedAlignedBuffer::<Align16>::with_capacity(10).into_shared();
    assert_eq!(shared.as_ptr(), empty_ptr);
    assert_eq!(shared.capacity(), 0);
    assert_eq!(shared.into_owned().as_ptr(), empty_ptr);

    // Empty buffer grows into an allocation
    let mut owned = OwnedAlignedBuffer::<Align16>::with_capacity(0);
    assert!(owned.append(b"abc"));
    assert_ne!(owned.as_ptr(), empty_ptr);
    assert_eq!(owned.as_slice(), b"abc");
}

#[test]
fn empty_buffers_do_not_touch_reference_counter() {
    extern crate std;

    use std::thread;

    let empty = SharedAlignedBuffer::<Align16>::default();
    thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| {
                for _ in 0..10 {
                    let clones = [
                        empty.clone(),
                        SharedAlignedBuffer::<Align16>::empty_ref().clone(),
                    ];
                    drop(clones);
                    drop(SharedAlignedBuffer::<Align16>::default().into_owned());
                }
            });
        }
    });

    assert_eq!(
        EMPTY_ALLOCATION.header.strong_count.load(Ordering::Relaxed),
        0
    );
    assert_eq!(EMPTY_ALLOCATION.header.capacity, 0);
}

fn check_allocation_alignment<A, AA>()
where
    A: Alignment,
    AA: AtLeast<A> + AtLeast<Align16>,
{
    let bytes = (0..=u8::MAX).cycle().take(1000).collect::<Vec<_>>();

    let mut owned = OwnedAlignedBuffer::<A>::with_capacity_aligned::<AA>(10);
    assert!(owned.capacity() >= 10);
    assert_eq!(owned.allocation_alignment(), AA::ALIGN);
    assert!(owned.as_byte_ptr().is_aligned_to(AA::ALIGN));

    // Growing preserves the alignment of the allocation
    for len in [1, 10, 11, 100, 1000] {
        owned.copy_from_slice(&bytes[..len]);
        assert!(owned.append(&bytes[..len]));
        assert_eq!(owned.allocation_alignment(), AA::ALIGN, "Length {len}");
        assert!(owned.as_byte_ptr().is_aligned_to(AA::ALIGN), "Length {len}");
        assert_eq!(owned.len() as usize, len * 2, "Length {len}");
        assert_eq!(&owned[..len], &bytes[..len], "Length {len}");
        assert_eq!(&owned[len..], &bytes[..len], "Length {len}");
    }
    owned.ensure_capacity(5000);
    assert_eq!(owned.allocation_alignment(), AA::ALIGN);
    assert!(owned.as_byte_ptr().is_aligned_to(AA::ALIGN));
    assert_eq!(&owned[..1000], &bytes[..1000]);

    // Copies have the same alignment
    let cloned = owned.clone();
    assert_eq!(cloned.allocation_alignment(), AA::ALIGN);
    assert!(cloned.as_byte_ptr().is_aligned_to(AA::ALIGN));
    assert_eq!(cloned.as_slice(), owned.as_slice());

    let ptr = owned.as_byte_ptr();
    let shared = owned.into_shared();
    let shared_clone = shared.clone();
    assert_eq!(shared.allocation_alignment(), AA::ALIGN);
    let copy = shared.into_owned();
    assert_ne!(copy.as_byte_ptr(), ptr);
    assert_eq!(copy.allocation_alignment(), AA::ALIGN);
    assert!(copy.as_byte_ptr().is_aligned_to(AA::ALIGN));
    assert_eq!(copy.as_slice(), shared_clone.as_slice());
    // The last reference reuses the allocation
    assert_eq!(shared_clone.into_owned().as_byte_ptr(), ptr);
}

#[test]
fn allocation_alignment() {
    check_allocation_alignment::<Align1, Align16>();
    check_allocation_alignment::<Align1, Align4096>();
    check_allocation_alignment::<Align16, Align16>();
    check_allocation_alignment::<Align16, Align32>();
    check_allocation_alignment::<Align32, Align32>();
    check_allocation_alignment::<Align32, Align4096>();
    check_allocation_alignment::<Align4096, Align4096>();
}

#[test]
fn empty_buffer_grows_to_alignment_of_contents() {
    let mut owned = OwnedAlignedBuffer::<Align1>::EMPTY;
    assert_eq!(owned.allocation_alignment(), Align16::ALIGN);
    assert!(owned.append(b"abc"));
    assert_eq!(owned.allocation_alignment(), Align16::ALIGN);

    let mut owned = OwnedAlignedBuffer::<Align4096>::EMPTY;
    assert!(owned.as_byte_ptr().is_aligned_to(Align4096::ALIGN));
    assert_eq!(owned.allocation_alignment(), Align4096::ALIGN);
    assert!(owned.append(b"abc"));
    assert_eq!(owned.allocation_alignment(), Align4096::ALIGN);
    assert!(owned.as_byte_ptr().is_aligned_to(Align4096::ALIGN));
    assert_eq!(owned.as_slice(), b"abc");

    // Zero capacity doesn't keep the alignment of the allocation
    let mut owned = OwnedAlignedBuffer::<Align1>::with_capacity_aligned::<Align4096>(0);
    assert_eq!(owned.allocation_alignment(), Align16::ALIGN);
    assert!(owned.append(b"abc"));
    assert_eq!(owned.allocation_alignment(), Align16::ALIGN);
    assert_eq!(
        SharedAlignedBuffer::<Align32>::EMPTY.allocation_alignment(),
        Align32::ALIGN
    );
}

#[test]
fn start_alignment_conversions() {
    let owned = OwnedAlignedBuffer::<Align16>::with_capacity_aligned::<Align4096>(10);
    let owned = owned
        .try_into_start_align::<Align4096>()
        .unwrap()
        .into_start_align::<Align1>();
    let owned = owned.try_into_start_align::<Align32>().unwrap();
    assert!(owned.as_byte_ptr().is_aligned_to(Align32::ALIGN));

    let shared = SharedAlignedBuffer::<Align16>::from_bytes(b"abc").into_start_align::<Align1>();
    let shared = shared.try_into_start_align::<Align16>().unwrap();
    assert!(shared.as_ptr().is_aligned_to(Align16::ALIGN));
    assert_eq!(shared.as_slice(), b"abc");

    // Allocations aligned to 16 bytes are rarely aligned to 4096 bytes
    let buffers = iter::repeat_with(|| OwnedAlignedBuffer::<Align16>::from_bytes(b"abc"))
        .take(10)
        .collect::<Vec<_>>();
    for owned in buffers {
        let aligned = owned.as_byte_ptr().is_aligned_to(Align4096::ALIGN);
        match owned.try_into_start_align::<Align4096>() {
            Ok(owned) => {
                assert!(aligned);
                assert_eq!(owned.as_slice(), b"abc");
            }
            Err(owned) => {
                assert!(!aligned);
                assert_eq!(owned.as_slice(), b"abc");
            }
        }
    }
}

#[test]
fn constructors_with_alignment_of_contents() {
    let owned = OwnedAlignedBuffer::<Align4096>::with_capacity(10);
    assert_eq!(owned.allocation_alignment(), Align4096::ALIGN);
    assert!(owned.as_byte_ptr().is_aligned_to(Align4096::ALIGN));

    let owned = OwnedAlignedBuffer::<Align32>::from_bytes(b"abc");
    assert_eq!(owned.allocation_alignment(), Align32::ALIGN);
    assert!(owned.as_byte_ptr().is_aligned_to(Align32::ALIGN));
    assert_eq!(owned.as_slice(), b"abc");

    let shared = SharedAlignedBuffer::<Align1>::from_bytes(b"abc");
    assert_eq!(shared.allocation_alignment(), Align16::ALIGN);
    assert_eq!(shared.as_slice(), b"abc");
}

#[test]
fn growth_preserves_alignment_of_contents() {
    // Allocation is only aligned to 16 bytes, but contents are moved within it to an address
    // aligned to 32 bytes
    let mut owned = OwnedAlignedBuffer::<Align16>::with_capacity(64);
    assert!(owned.append(b"abc"));
    let mut owned = owned.realign::<Align32>();
    assert_eq!(owned.allocation_alignment(), Align16::ALIGN);
    assert!(owned.as_byte_ptr().is_aligned_to(Align32::ALIGN));

    owned.ensure_capacity(1000);
    assert_eq!(owned.allocation_alignment(), Align32::ALIGN);
    assert!(owned.as_byte_ptr().is_aligned_to(Align32::ALIGN));
    assert_eq!(owned.as_slice(), b"abc");
}

#[test]
fn slices() {
    let bytes = (0..100).collect::<Vec<u8>>();
    let shared = SharedAlignedBuffer::<Align16>::from_bytes(&bytes);

    assert_eq!(shared.get_slice(..).unwrap().as_slice(), bytes);
    assert_eq!(shared.get_slice(10..20).unwrap().as_slice(), &bytes[10..20]);
    assert_eq!(
        shared.get_slice(10..=20).unwrap().as_slice(),
        &bytes[10..=20]
    );
    assert_eq!(shared.get_slice(90..).unwrap().as_slice(), &bytes[90..]);
    assert_eq!(shared.get_slice(..5).unwrap().as_slice(), &bytes[..5]);
    assert!(shared.get_slice(..101).is_none());
    assert!(shared.get_slice(50..101).is_none());
    assert!(shared.get_slice(101..).is_none());
    #[expect(clippy::reversed_empty_ranges, reason = "Testing invalid range")]
    let reversed = shared.get_slice(20..10);
    assert!(reversed.is_none());
    assert!(shared.get_slice(..=u32::MAX).is_none());

    // Empty slices don't hold a reference
    let empty = shared.get_slice(50..50).unwrap();
    assert!(empty.is_empty());
    assert_eq!(
        empty.as_byte_ptr(),
        SharedAlignedBuffer::<Align1>::EMPTY.as_byte_ptr()
    );

    let slice = shared.get_slice(7..30).unwrap();
    // SAFETY: Within the contents
    assert_eq!(slice.as_byte_ptr(), unsafe { shared.as_byte_ptr().add(7) });
    assert_eq!(slice.offset(), 7);
    assert_eq!(slice.capacity(), shared.capacity() - 7);
    assert_eq!(slice.allocation_alignment(), shared.allocation_alignment());

    // Slices of slices
    let slice_of_slice = slice.get_slice(3..10).unwrap();
    assert_eq!(slice_of_slice.as_slice(), &bytes[10..17]);
    assert_eq!(slice_of_slice.offset(), 10);
    assert!(slice.get_slice(..24).is_none());

    // Slices keep the allocation alive
    drop(shared);
    drop(slice);
    assert_eq!(slice_of_slice.as_slice(), &bytes[10..17]);
    let cloned = slice_of_slice.clone();
    assert_eq!(cloned.as_byte_ptr(), slice_of_slice.as_byte_ptr());
    assert_eq!(cloned.offset(), 10);
}

#[test]
fn owned_slices() {
    let bytes = (0..100).collect::<Vec<u8>>();
    let shared = SharedAlignedBuffer::<Align16>::from_bytes(&bytes);
    let mut slice = shared.get_slice(16..48).unwrap();

    // Shared with the original buffer
    slice = slice.try_into_owned().unwrap_err();
    let copy = slice.clone().into_owned();
    assert_eq!(copy.as_slice(), &bytes[16..48]);
    assert_eq!(copy.offset(), 0);

    // The last reference reuses the allocation with the same offset
    drop(shared);
    let ptr = slice.as_byte_ptr();
    let mut owned = slice.try_into_owned().unwrap();
    assert_eq!(owned.as_byte_ptr(), ptr);
    assert_eq!(owned.offset(), 16);
    assert_eq!(owned.capacity(), 100 - 16);
    assert_eq!(owned.as_slice(), &bytes[16..48]);

    // Bytes after the contents are available as capacity
    assert!(owned.append(&bytes[..50]));
    assert_eq!(owned.as_byte_ptr(), ptr);
    assert_eq!(&owned[32..], &bytes[..50]);

    // Growing keeps the offset
    assert!(owned.append(&bytes));
    assert_eq!(owned.offset(), 16);
    assert_eq!(owned.len(), 32 + 50 + 100);
    assert_eq!(&owned[..32], &bytes[16..48]);
    assert_eq!(&owned[82..], &bytes);
    assert!(owned.as_ptr().is_aligned_to(EXPECTED_ALIGNMENT));

    // Truncated to empty slice doesn't hold a reference anymore
    let shared = owned.into_shared();
    let mut slice = shared.get_slice(1..).unwrap();
    slice.truncate(10);
    assert_eq!(slice.len(), 10);
    let shared = shared.try_into_owned().unwrap_err();
    slice.truncate(0);
    assert!(slice.is_empty());
    shared.try_into_owned().unwrap();
}

#[test]
fn standalone() {
    let bytes = (0..100).collect::<Vec<u8>>();
    let shared = SharedAlignedBuffer::<Align16>::from_bytes(&bytes);
    let ptr = shared.as_byte_ptr();

    // The whole allocation is already standalone
    let shared = shared.into_standalone();
    assert_eq!(shared.as_byte_ptr(), ptr);

    let slice = shared
        .get_slice(32..48)
        .unwrap()
        .try_into_start_align::<Align16>()
        .unwrap()
        .into_standalone();
    assert_ne!(slice.as_byte_ptr(), ptr);
    assert_eq!(slice.offset(), 0);
    assert_eq!(slice.capacity(), 16);
    assert_eq!(slice.as_slice(), &bytes[32..48]);
    assert!(slice.as_ptr().is_aligned_to(EXPECTED_ALIGNMENT));
    // The original allocation is not referenced anymore
    let owned = shared.try_into_owned().unwrap();
    assert_eq!(owned.as_byte_ptr(), ptr);

    // Owned buffer with spare capacity is not standalone
    let mut owned = OwnedAlignedBuffer::<Align16>::with_capacity(100);
    assert!(owned.append(&bytes[..10]));
    let ptr = owned.as_byte_ptr();
    let shared = owned.into_shared().into_standalone();
    assert_ne!(shared.as_byte_ptr(), ptr);
    assert_eq!(shared.capacity(), 10);

    assert!(
        SharedAlignedBuffer::<Align16>::EMPTY
            .into_standalone()
            .is_empty()
    );
}

#[test]
fn relocate() {
    let bytes = (0..100).collect::<Vec<u8>>();

    // Within the allocation, forward and backward with overlapping ranges
    let mut owned = OwnedAlignedBuffer::<Align1>::with_capacity_aligned::<Align16>(200);
    let ptr = owned.as_byte_ptr();
    assert!(owned.append(&bytes));
    assert!(owned.relocate::<Align16>(10, 100));
    assert_eq!(owned.offset(), 10);
    // SAFETY: Within the allocation
    assert_eq!(owned.as_byte_ptr(), unsafe { ptr.add(10) });
    assert_eq!(owned.capacity(), 190);
    assert_eq!(owned.as_slice(), bytes);
    assert!(owned.relocate::<Align16>(3, 100));
    assert_eq!(owned.offset(), 3);
    assert_eq!(owned.as_slice(), bytes);
    assert!(owned.relocate::<Align16>(0, 0));
    assert_eq!(owned.as_byte_ptr(), ptr);
    assert_eq!(owned.as_slice(), bytes);

    // Not enough capacity or alignment, copied into a new allocation
    assert!(owned.relocate::<Align16>(150, 100));
    assert_ne!(owned.as_byte_ptr(), ptr);
    assert_eq!(owned.offset(), 150);
    assert_eq!(owned.capacity(), 100);
    assert_eq!(owned.as_slice(), bytes);
    assert!(owned.relocate::<Align4096>(1, 0));
    assert_eq!(owned.allocation_alignment(), Align4096::ALIGN);
    assert_eq!(owned.as_byte_ptr().addr() % Align4096::ALIGN, 1);
    assert_eq!(owned.capacity(), 100);
    assert_eq!(owned.as_slice(), bytes);

    // Invalid offset or capacity leaves the buffer unchanged
    let mut owned = OwnedAlignedBuffer::<Align16>::from_bytes(&bytes);
    let ptr = owned.as_byte_ptr();
    assert!(!owned.relocate::<Align16>(1, 0));
    assert!(!owned.relocate::<Align16>(16, u32::MAX));
    assert_eq!(owned.as_byte_ptr(), ptr);
    assert_eq!(owned.as_slice(), bytes);
    assert!(owned.relocate::<Align16>(16, 0));
    assert!(owned.as_ptr().is_aligned_to(EXPECTED_ALIGNMENT));
    assert_eq!(owned.as_slice(), bytes);

    // No capacity after the offset, but still backed by an allocation that must be freed
    let mut owned = OwnedAlignedBuffer::<Align1>::EMPTY;
    assert!(owned.relocate::<Align16>(10, 0));
    assert_eq!(owned.offset(), 10);
    assert_eq!(owned.capacity(), 0);
    drop(owned);

    // Empty buffer for writing contents at an offset
    let mut owned = OwnedAlignedBuffer::<Align1>::EMPTY;
    assert!(owned.relocate::<Align4096>(100, 8000));
    assert_eq!(owned.offset(), 100);
    assert_eq!(owned.as_byte_ptr().addr() % Align4096::ALIGN, 100);
    assert!(owned.capacity() >= 8000);
    assert!(owned.is_empty());
}

#[test]
fn realign() {
    let bytes = (0..100).collect::<Vec<u8>>();

    // Already aligned
    let owned = OwnedAlignedBuffer::<Align16>::from_bytes(&bytes).into_start_align::<Align1>();
    let ptr = owned.as_byte_ptr();
    let owned = owned.realign::<Align16>();
    assert_eq!(owned.as_byte_ptr(), ptr);

    // Moved backward within the allocation
    let shared = owned.into_shared();
    let owned = shared
        .get_slice(19..)
        .unwrap()
        .try_into_owned()
        .unwrap_err();
    drop(shared);
    // To the closest aligned offset
    let owned = owned.try_into_owned().unwrap().realign::<Align16>();
    // SAFETY: Within the allocation
    assert_eq!(owned.as_byte_ptr(), unsafe { ptr.add(16) });
    assert_eq!(owned.offset(), 16);
    assert_eq!(owned.as_slice(), &bytes[19..]);

    // Allocations aligned to 16 bytes, but not to 32 bytes, buffers are kept alive, so that their
    // addresses are different
    let mut buffers =
        iter::repeat_with(|| OwnedAlignedBuffer::<Align1>::with_capacity_aligned::<Align16>(100))
            .take(100)
            .collect::<Vec<_>>();
    let mut misaligned_buffers = iter::from_fn(|| {
        let index = buffers
            .iter()
            .position(|owned| !owned.as_byte_ptr().is_aligned_to(Align32::ALIGN))?;
        Some(buffers.swap_remove(index))
    });

    // Moved forward within the allocation
    let mut owned = misaligned_buffers.next().unwrap();
    let ptr = owned.as_byte_ptr();
    assert!(owned.append(&bytes[..10]));
    let owned = owned.realign::<Align32>();
    // SAFETY: Within the allocation
    assert_eq!(owned.as_byte_ptr(), unsafe { ptr.add(16) });
    assert_eq!(owned.offset(), 16);
    assert_eq!(owned.as_slice(), &bytes[..10]);

    // Doesn't fit, copied into a new allocation
    let mut owned = misaligned_buffers.next().unwrap();
    let ptr = owned.as_byte_ptr();
    assert!(owned.append(&bytes[..90]));
    let owned = owned.realign::<Align32>();
    assert_ne!(owned.as_byte_ptr(), ptr);
    assert_eq!(owned.offset(), 0);
    assert!(owned.as_byte_ptr().is_aligned_to(Align32::ALIGN));
    assert_eq!(owned.as_slice(), &bytes[..90]);

    let owned = owned.into_start_align::<Align1>().realign::<Align4096>();
    assert!(owned.as_byte_ptr().is_aligned_to(Align4096::ALIGN));
    assert_eq!(owned.as_slice(), &bytes[..90]);
}

#[test]
fn last_reference_synchronizes_with_other_threads() {
    extern crate std;

    use std::thread;

    let mut shared = SharedAlignedBuffer::<Align16>::from_bytes(&[1; 64]);
    let slice = shared.get_slice(16..32).unwrap();
    // Not joined, so synchronization comes from the reference counter alone
    let _handle = thread::spawn(move || {
        assert_eq!(slice.as_slice(), [1; 16]);
    });

    let mut owned = loop {
        match shared.try_into_owned() {
            Ok(owned) => {
                break owned;
            }
            Err(not_unique) => {
                shared = not_unique;
                thread::yield_now();
            }
        }
    };
    owned.as_mut_slice().fill(2);
    assert_eq!(owned.as_slice(), [2; 64]);
}

#[test]
fn zeroed() {
    for len in [0, 1, 15, 16, 17, 1000] {
        let owned = OwnedAlignedBuffer::<Align16>::zeroed(len);
        assert_eq!(owned.len(), len, "Length {len}");
        assert!(owned.capacity() >= len, "Length {len}");
        assert!(owned.iter().all(|&byte| byte == 0), "Length {len}");
        assert!(
            owned.as_ptr().is_aligned_to(EXPECTED_ALIGNMENT),
            "Length {len}"
        );

        let owned = OwnedAlignedBuffer::<Align1>::zeroed_aligned::<Align4096>(len);
        assert_eq!(owned.len(), len, "Length {len}");
        assert!(owned.iter().all(|&byte| byte == 0), "Length {len}");
        if len != 0 {
            assert_eq!(
                owned.allocation_alignment(),
                Align4096::ALIGN,
                "Length {len}"
            );
        }
    }
}

#[test]
fn spare_capacity() {
    let mut owned = OwnedAlignedBuffer::<Align16>::with_capacity(10);
    assert!(owned.append(b"abc"));
    let capacity = owned.capacity();
    let spare_capacity = owned.spare_capacity_mut();
    assert_eq!(spare_capacity.len() as u32, capacity - 3);
    spare_capacity[..4].write_copy_of_slice(b"defg");
    // SAFETY: Initialized 4 more bytes above
    unsafe {
        owned.set_len(7);
    }
    assert_eq!(owned.as_slice(), b"abcdefg");

    let mut empty = OwnedAlignedBuffer::<Align1>::EMPTY;
    assert!(empty.spare_capacity_mut().is_empty());
}

fn check_aligned_buffer<B>(buffer: B, expected: &[u8], offset: u32)
where
    B: AlignedBuffer<Alignment = Align1>,
{
    assert_eq!(&*buffer, expected);
    assert_eq!(AlignedBuffer::len(&buffer) as usize, expected.len());
    assert_eq!(AlignedBuffer::is_empty(&buffer), expected.is_empty());
    assert_eq!(AlignedBuffer::offset(&buffer), offset);
    assert_eq!(
        AlignedBuffer::allocation_alignment(&buffer),
        Align4096::ALIGN
    );
    let ptr = buffer.as_ptr();
    let shared = AlignedBuffer::into_shared(buffer);
    assert_eq!(shared.as_byte_ptr(), ptr);
    assert_eq!(shared.as_slice(), expected);
}

#[test]
fn aligned_buffer() {
    let mut owned = OwnedAlignedBuffer::<Align1>::with_capacity_aligned::<Align4096>(100);
    assert!(owned.relocate::<Align4096>(10, 0));
    assert!(owned.append(b"abc"));
    check_aligned_buffer(owned.clone(), b"abc", 0);
    let shared = owned.into_shared();
    check_aligned_buffer(shared.clone(), b"abc", 10);
    check_aligned_buffer(shared.get_slice(1..).unwrap(), b"bc", 11);
}

// A `u32` capacity can only exceed `isize::MAX` bytes where `usize` is 32-bit
#[cfg(target_pointer_width = "32")]
#[test]
#[should_panic(expected = "has failed: too large")]
fn capacity_overflow_panics() {
    OwnedAlignedBuffer::<Align16>::with_capacity(1).ensure_capacity(u32::MAX);
}

// A `u32` capacity can only exceed `isize::MAX` bytes where `usize` is 32-bit
#[cfg(target_pointer_width = "32")]
#[test]
#[should_panic(expected = "has failed: too large")]
fn capacity_above_isize_max_panics() {
    OwnedAlignedBuffer::<Align16>::with_capacity(1).ensure_capacity(1 << 31);
}

// The largest capacity is only below `u32::MAX` where `usize` is 32-bit
#[cfg(target_pointer_width = "32")]
#[test]
#[should_panic(expected = "has failed: too large")]
fn capacity_above_max_capacity_panics() {
    OwnedAlignedBuffer::<Align16>::with_capacity(1)
        .ensure_capacity(OwnedAlignedBuffer::<Align16>::MAX_CAPACITY + 1);
}

// The largest capacity is only below `u32::MAX` where `usize` is 32-bit, and gigabytes of memory
// are too much for Miri
#[cfg(all(target_pointer_width = "32", not(miri)))]
#[test]
fn append_beyond_max_capacity() {
    let mut owned = OwnedAlignedBuffer::<Align16>::from_bytes(&[1, 2, 3]);
    // Zeroed allocation is not backed by memory until written to
    let bytes = vec![0; OwnedAlignedBuffer::<Align16>::MAX_CAPACITY as usize - 2];

    assert!(!owned.append(&bytes));
    assert_eq!(owned.as_slice(), [1, 2, 3]);
}

// The largest capacity is only below `u32::MAX` where `usize` is 32-bit
#[cfg(target_pointer_width = "32")]
#[test]
#[should_panic(expected = "has failed: too large")]
fn capacity_with_offset_above_max_capacity_panics() {
    let mut owned = OwnedAlignedBuffer::<Align1>::EMPTY;
    assert!(owned.relocate::<Align16>(16, 1));
    owned.ensure_capacity(OwnedAlignedBuffer::<Align16>::MAX_CAPACITY - 15);
}

// The largest capacity is only below `u32::MAX` where `usize` is 32-bit, and gigabytes of memory
// are too much for Miri
#[cfg(all(target_pointer_width = "32", not(miri)))]
#[test]
fn append_with_offset_beyond_max_capacity() {
    let mut owned = OwnedAlignedBuffer::<Align1>::EMPTY;
    assert!(owned.relocate::<Align16>(16, 3));
    assert!(owned.append(&[1, 2, 3]));
    // Zeroed allocation is not backed by memory until written to
    let bytes = vec![0; OwnedAlignedBuffer::<Align16>::MAX_CAPACITY as usize - 3 - 15];

    assert!(!owned.append(&bytes));
    assert_eq!(owned.as_slice(), [1, 2, 3]);
}

// Only a counter narrower than 64 bits can get close to wrapping around
#[cfg(target_pointer_width = "32")]
#[test]
fn too_many_clones_panic() {
    extern crate std;

    use core::sync::atomic::Ordering;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    let shared = SharedAlignedBuffer::<Align16>::from_bytes(b"abc");
    let strong_count = shared.inner.strong_count_ref();
    // As if `isize::MAX - 1` clones were leaked
    strong_count.store(isize::MAX.cast_unsigned(), Ordering::Relaxed);

    let clone = shared.clone();
    catch_unwind(AssertUnwindSafe(|| shared.clone())).unwrap_err();
    assert_eq!(
        strong_count.load(Ordering::Relaxed),
        isize::MAX.cast_unsigned() + 1
    );

    // Only the two instances that are alive remain, so dropping them frees the buffer
    strong_count.store(2, Ordering::Relaxed);
    drop(clone);
}
