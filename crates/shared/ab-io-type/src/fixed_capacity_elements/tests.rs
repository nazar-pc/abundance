use crate::fixed_capacity_bytes::FixedCapacityBytes;
use crate::fixed_capacity_elements::FixedCapacityElements;
use crate::trivial_type::TrivialType;

#[test]
fn try_from_slice() {
    let bytes = FixedCapacityBytes::<4>::try_from_slice(&[1, 2, 3]).unwrap();
    assert_eq!(bytes.len(), 3);
    assert!(!bytes.is_empty());
    assert_eq!(bytes.get_elements(), &[1, 2, 3]);

    let bytes = FixedCapacityBytes::<4>::try_from_slice(&[]).unwrap();
    assert_eq!(bytes.len(), 0);
    assert!(bytes.is_empty());
    assert_eq!(bytes.get_elements(), &[]);

    let bytes = FixedCapacityBytes::<4>::try_from_slice(&[1, 2, 3, 4]).unwrap();
    assert_eq!(bytes.get_elements(), &[1, 2, 3, 4]);

    // Not enough capacity
    assert!(FixedCapacityBytes::<4>::try_from_slice(&[1, 2, 3, 4, 5]).is_none());
    assert!(FixedCapacityBytes::<0>::try_from_slice(&[1]).is_none());

    let elements = FixedCapacityElements::<[u16; 3], 3>::try_from_slice(&[[1, 2, 3]]).unwrap();
    assert_eq!(elements.len(), 1);
    assert_eq!(elements.get_elements(), &[[1, 2, 3]]);
}

#[test]
fn default() {
    let bytes = FixedCapacityBytes::<4>::default();
    assert!(bytes.is_empty());
    assert_eq!(bytes.as_bytes(), &[0; 8]);

    let elements = FixedCapacityElements::<u128, 2>::default();
    assert!(elements.is_empty());
    assert_eq!(elements.as_bytes(), &[0; 48]);
}

#[test]
fn get_elements_mut() {
    let mut elements = FixedCapacityElements::<u64, 4>::try_from_slice(&[1, 2]).unwrap();

    elements.get_elements_mut()[1] = 3;
    assert_eq!(elements.get_elements(), &[1, 3]);
    assert_eq!(elements.get_elements_mut(), &mut [1, 3]);
}

#[test]
fn append() {
    let mut bytes = FixedCapacityBytes::<4>::try_from_slice(&[1]).unwrap();

    assert!(bytes.append(&[2, 3]));
    assert_eq!(bytes.get_elements(), &[1, 2, 3]);

    // Not enough capacity
    assert!(!bytes.append(&[4, 5]));
    assert_eq!(bytes.get_elements(), &[1, 2, 3]);

    assert!(bytes.append(&[4]));
    assert_eq!(bytes.get_elements(), &[1, 2, 3, 4]);

    assert!(bytes.append(&[]));
    assert_eq!(bytes.get_elements(), &[1, 2, 3, 4]);
    assert!(!bytes.append(&[5]));

    let mut elements = FixedCapacityElements::<u128, 3>::default();
    assert!(elements.append(&[1, 2]));
    assert!(!elements.append(&[3, 4]));
    assert!(elements.append(&[3]));
    assert_eq!(elements.get_elements(), &[1, 2, 3]);
}

#[test]
fn truncate() {
    let mut elements = FixedCapacityElements::<u32, 4>::try_from_slice(&[1, 2, 3]).unwrap();

    // Larger than the length
    assert!(!elements.truncate(4));
    assert_eq!(elements.get_elements(), &[1, 2, 3]);

    assert!(elements.truncate(3));
    assert_eq!(elements.get_elements(), &[1, 2, 3]);

    assert!(elements.truncate(1));
    assert_eq!(elements.get_elements(), &[1]);

    assert!(elements.truncate(0));
    assert!(elements.is_empty());
    assert_eq!(elements.get_elements(), &[]);
}

#[test]
fn copy_from() {
    let mut bytes = FixedCapacityBytes::<4>::try_from_slice(&[1]).unwrap();

    assert!(bytes.copy_from(&[2, 3]));
    assert_eq!(bytes.get_elements(), &[2, 3]);

    // Not enough capacity
    assert!(!bytes.copy_from(&[4, 5, 6, 7, 8]));
    assert_eq!(bytes.get_elements(), &[2, 3]);

    assert!(bytes.copy_from(&[4, 5, 6, 7]));
    assert_eq!(bytes.get_elements(), &[4, 5, 6, 7]);

    assert!(bytes.copy_from(&[]));
    assert!(bytes.is_empty());

    let mut elements = FixedCapacityElements::<u64, 2>::try_from_slice(&[1]).unwrap();
    assert!(elements.copy_from(&[2, 3]));
    assert_eq!(elements.get_elements(), &[2, 3]);
    assert!(!elements.copy_from(&[4, 5, 6]));
    assert_eq!(elements.get_elements(), &[2, 3]);
}

#[test]
fn length_prefix_padding_is_zeroed() {
    // Elements with alignment 8 extend the length prefix to 8 bytes
    let mut elements = FixedCapacityElements::<u64, 2>::try_from_slice(&[1, 2]).unwrap();

    // Non-zero bytes after the length, like in an instance created from arbitrary bytes
    elements.len[4..].fill(0xff);
    assert!(elements.truncate(1));
    assert_eq!(elements.as_bytes()[..8], [1, 0, 0, 0, 0, 0, 0, 0]);

    elements.len[4..].fill(0xff);
    assert!(elements.copy_from(&[3, 4]));
    assert_eq!(elements.as_bytes()[..8], [2, 0, 0, 0, 0, 0, 0, 0]);

    elements.len[4..].fill(0xff);
    assert!(elements.truncate(0));
    assert!(elements.append(&[5]));
    assert_eq!(elements.as_bytes()[..8], [1, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(elements.get_elements(), &[5]);
}

#[test]
fn length_above_capacity() {
    // Bytes of an instance with capacity 4 and length 5
    let bytes = [5, 0, 0, 0, 1, 2, 3, 4];
    // SAFETY: `FixedCapacityBytes<4>` has alignment 1, `bytes` are its size and any bytes of that
    // size are a valid value of it, an invalid length is what is tested
    let mut invalid = *unsafe { FixedCapacityBytes::<4>::from_bytes_unchecked(&bytes) };

    // The length is not checked, but no elements are accessible and none can be appended
    assert_eq!(invalid.len(), 5);
    assert!(!invalid.is_empty());
    assert_eq!(invalid.get_elements(), &[]);
    assert_eq!(invalid.get_elements_mut(), &mut []);
    assert!(!invalid.append(&[]));
    assert!(!invalid.append(&[5]));
    assert_eq!(invalid.len(), 5);

    // Both make the length valid again
    let mut truncated = invalid;
    assert!(truncated.truncate(2));
    assert_eq!(truncated.get_elements(), &[1, 2]);
    let mut copied = invalid;
    assert!(copied.copy_from(&[6, 7, 8]));
    assert_eq!(copied.get_elements(), &[6, 7, 8]);

    // The largest length
    let bytes = [0xff, 0xff, 0xff, 0xff, 1, 2, 3, 4];
    // SAFETY: `FixedCapacityBytes<4>` has alignment 1, `bytes` are its size and any bytes of that
    // size are a valid value of it, an invalid length is what is tested
    let mut invalid = *unsafe { FixedCapacityBytes::<4>::from_bytes_unchecked(&bytes) };
    assert_eq!(invalid.len(), u32::MAX);
    assert_eq!(invalid.get_elements(), &[]);
    assert_eq!(invalid.get_elements_mut(), &mut []);
    assert!(!invalid.append(&[]));
    assert!(!invalid.append(&[5]));
}
