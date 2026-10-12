use crate::fixed_capacity_string::FixedCapacityString;

#[test]
fn try_from_str() {
    let string = FixedCapacityString::<4>::try_from_str("abc").unwrap();
    assert_eq!(string.len(), 3);
    assert_eq!(string.get_elements(), b"abc");

    let string = FixedCapacityString::<4>::try_from_str("").unwrap();
    assert!(string.is_empty());

    // The capacity is in bytes, not characters
    let string = FixedCapacityString::<4>::try_from_str("ab¢").unwrap();
    assert_eq!(string.get_elements(), "ab¢".as_bytes());
    assert!(FixedCapacityString::<4>::try_from_str("abc¢").is_none());
    assert!(FixedCapacityString::<4>::try_from_str("abcde").is_none());
}

#[test]
fn try_from_slice() {
    let string = FixedCapacityString::<4>::try_from_slice(b"abcd").unwrap();
    assert_eq!(string.get_elements(), b"abcd");

    // Contents is not checked to be UTF-8
    let string = FixedCapacityString::<4>::try_from_slice(&[0xff]).unwrap();
    assert_eq!(string.get_elements(), &[0xff]);

    assert!(FixedCapacityString::<4>::try_from_slice(b"abcde").is_none());
}

#[test]
fn deref() {
    let mut string = FixedCapacityString::<4>::default();
    assert!(string.is_empty());

    assert!(string.append(b"ab"));
    assert!(string.append(b"c"));
    assert!(!string.append(b"de"));
    assert_eq!(string.get_elements(), b"abc");

    string.get_elements_mut()[0] = b'x';
    assert_eq!(string.get_elements(), b"xbc");

    assert!(string.truncate(1));
    assert_eq!(string.get_elements(), b"x");

    assert!(string.copy_from(b"yz"));
    assert_eq!(string.get_elements(), b"yz");
}
