use crate::{AlignedBytes, OwnedAlignedBuffer, SharedAlignedBuffer};
use alloc::vec;

const EXPECTED_ALIGNMENT: usize = size_of::<AlignedBytes>();

#[test]
fn basic() {
    for capacity in 0..=EXPECTED_ALIGNMENT as u32 {
        let mut owned = OwnedAlignedBuffer::with_capacity(capacity);
        assert_eq!(owned.len(), 0, "Capacity {capacity}");
        assert!(owned.capacity() >= capacity, "Capacity {capacity}");
        assert!(owned.is_empty(), "Capacity {capacity}");
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
            assert_eq!(owned.as_slice().len(), len as usize, "Capacity {capacity}");
            assert_eq!(
                owned.as_mut_slice().len(),
                len as usize,
                "Capacity {capacity}"
            );
            assert!(owned.capacity() >= capacity, "Capacity {capacity}");
            if len != 0 {
                assert!(!owned.is_empty(), "Capacity {capacity}");
                assert!(!owned.as_slice().is_empty(), "Capacity {capacity}");
                assert!(!owned.as_mut_slice().is_empty(), "Capacity {capacity}");
            }
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
            assert_eq!(owned.as_slice().len(), len as usize, "Capacity {capacity}");
            assert_eq!(
                owned.as_mut_slice().len(),
                len as usize,
                "Capacity {capacity}"
            );
            assert!(owned.capacity() >= capacity, "Capacity {capacity}");
            if len != 0 {
                assert!(!owned.is_empty(), "Capacity {capacity}");
                assert!(!owned.as_slice().is_empty(), "Capacity {capacity}");
                assert!(!owned.as_mut_slice().is_empty(), "Capacity {capacity}");
            }
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
            assert_eq!(owned.as_slice().len(), len as usize, "Capacity {capacity}");
            assert_eq!(
                owned.as_mut_slice().len(),
                len as usize,
                "Capacity {capacity}"
            );
            assert!(owned.capacity() >= capacity, "Capacity {capacity}");
            assert!(!owned.is_empty(), "Capacity {capacity}");
            assert!(!owned.as_slice().is_empty(), "Capacity {capacity}");
            assert!(!owned.as_mut_slice().is_empty(), "Capacity {capacity}");
            assert_eq!(owned.as_ptr(), owned.as_mut_ptr(), "Capacity {capacity}");
            assert_ne!(owned.as_ptr(), ptr_before, "Capacity {capacity}");
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
            assert_eq!(owned.as_slice(), owned2.as_slice(), "Capacity {capacity}");
            assert_eq!(owned.as_slice(), shared.as_slice(), "Capacity {capacity}");
            assert_eq!(
                owned.as_mut_slice(),
                owned2.as_mut_slice(),
                "Capacity {capacity}"
            );

            let shorter_len = owned2.len() - 1;
            // SAFETY: length is guaranteed to be within stored bytes
            unsafe { owned2.set_len(shorter_len) };
            assert_eq!(&owned.as_slice()[..shorter_len as usize], owned2.as_slice());
        }

        // Create a shared instance
        let shared = owned.into_shared();
        let ptr_before = shared.as_ptr();
        // Turn back into owned and confirm that it points to the same memory (meaning no additional
        // allocation)
        let owned = shared.into_owned();
        assert_eq!(owned.as_ptr(), ptr_before, "Capacity {capacity}");

        let shared = owned.into_shared();
        // Cloned shared instance will result in new allocation
        let owned = shared.clone().into_owned();
        assert_ne!(owned.as_ptr(), ptr_before, "Capacity {capacity}");

        let shared2 = shared.clone();
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
