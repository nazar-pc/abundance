use crate::alignment::{Align1, Align16, Align32, Align4096, Alignment, AtLeast};
use crate::{EMPTY_ALLOCATION, OwnedAlignedBuffer, SharedAlignedBuffer};
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
    // Allocation is only aligned to 16 bytes, but contents may happen to be aligned to 32 bytes
    // depending on the allocator
    // Buffers are kept alive, so that their addresses are different
    let mut buffers = iter::repeat_with(|| OwnedAlignedBuffer::<Align16>::from_bytes(b"abc"))
        .take(100)
        .collect::<Vec<_>>();
    let Some(index) = buffers
        .iter()
        .position(|owned| owned.as_byte_ptr().is_aligned_to(Align32::ALIGN))
    else {
        // Allocator never placed contents at an address aligned to 32 bytes
        return;
    };
    let owned = buffers
        .swap_remove(index)
        .try_into_start_align::<Align32>()
        .unwrap();
    assert_eq!(owned.allocation_alignment(), Align16::ALIGN);

    let mut owned = owned;
    owned.ensure_capacity(1000);
    assert_eq!(owned.allocation_alignment(), Align32::ALIGN);
    assert!(owned.as_byte_ptr().is_aligned_to(Align32::ALIGN));
    assert_eq!(owned.as_slice(), b"abc");
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
