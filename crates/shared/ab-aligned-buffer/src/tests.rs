use crate::{OwnedAlignedBuffer, SharedAlignedBuffer};
use alloc::vec;
use alloc::vec::Vec;
use core::iter;
use core::ops::{Deref, DerefMut};

const EXPECTED_ALIGNMENT: usize = 16;

#[test]
#[expect(
    clippy::explicit_deref_methods,
    reason = "Intentionally explicit for testing"
)]
fn basic() {
    for capacity in 0..=EXPECTED_ALIGNMENT as u32 {
        let mut owned = OwnedAlignedBuffer::with_capacity(capacity);
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

            let mut owned2 = OwnedAlignedBuffer::from_bytes(&bytes);
            let shared = SharedAlignedBuffer::from_bytes(&bytes);
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

            let mut owned2 = OwnedAlignedBuffer::from_bytes(&bytes);
            let shared = SharedAlignedBuffer::from_bytes(&bytes);
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

            let mut owned2 = OwnedAlignedBuffer::from_bytes(&bytes);
            let shared = SharedAlignedBuffer::from_bytes(&bytes);
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
        let shared = owned.into_shared();
        assert_eq!(shared.as_slice(), shared.deref(), "Capacity {capacity}");
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
    let mut owned = OwnedAlignedBuffer::with_capacity(10);
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
        let owned = OwnedAlignedBuffer::from_bytes(&bytes[..len]);
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
    let mut owned = OwnedAlignedBuffer::with_capacity(1);
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
    let empty = SharedAlignedBuffer::default();
    let shared = SharedAlignedBuffer::from_bytes(b"abc");
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
    assert!(SharedAlignedBuffer::empty_ref().is_empty());
    assert!(SharedAlignedBuffer::default().into_owned().is_empty());

    // The last instance reuses its allocation
    let owned = shared.into_owned();
    assert_eq!(owned.as_ptr(), ptr_before);
    assert_eq!(owned.as_slice(), b"abc");
}

// A `u32` capacity can only exceed `isize::MAX` bytes where `usize` is 32-bit
#[cfg(target_pointer_width = "32")]
#[test]
#[should_panic(expected = "has failed: too large")]
fn capacity_overflow_panics() {
    OwnedAlignedBuffer::with_capacity(1).ensure_capacity(u32::MAX);
}

// A `u32` capacity can only exceed `isize::MAX` bytes where `usize` is 32-bit
#[cfg(target_pointer_width = "32")]
#[test]
#[should_panic(expected = "has failed: too large")]
fn capacity_above_isize_max_panics() {
    OwnedAlignedBuffer::with_capacity(1).ensure_capacity(1 << 31);
}

// Only a counter narrower than 64 bits can get close to wrapping around
#[cfg(target_pointer_width = "32")]
#[test]
fn too_many_clones_panic() {
    extern crate std;

    use core::sync::atomic::Ordering;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    let shared = SharedAlignedBuffer::from_bytes(b"abc");
    let strong_count = shared.inner.strong_count_ref();
    // As if `isize::MAX - 1` clones were leaked
    strong_count.store(isize::MAX.cast_unsigned(), Ordering::Relaxed);

    let clone = shared.clone();
    assert!(catch_unwind(AssertUnwindSafe(|| shared.clone())).is_err());
    assert_eq!(
        strong_count.load(Ordering::Relaxed),
        isize::MAX.cast_unsigned() + 1
    );

    // Only the two instances that are alive remain, so dropping them frees the buffer
    strong_count.store(2, Ordering::Relaxed);
    drop(clone);
}
