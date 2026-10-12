//! Tests of [`MethodFingerprint`]

#![expect(dead_code, reason = "Types are only defined for their metadata")]

use ab_contracts_common::metadata::ContractMetadataKind;
use ab_contracts_common::method::MethodFingerprint;
use ab_io_type::trivial_type::TrivialType;

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
struct Named {
    first: u8,
    second: [u8; 2],
}

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
struct Tuple(u8, [u8; 2]);

#[derive(Copy, Clone, TrivialType)]
#[repr(u8)]
enum EnumOfNamed {
    Value { value: Named },
}

#[derive(Copy, Clone, TrivialType)]
#[repr(u8)]
enum EnumOfTuple {
    Other { other: Tuple },
}

/// Metadata of an `#[update]` method `method_name` with a single `#[input]` argument of type `T`
fn update_method_metadata<T>(method_name: &str, argument_name: &str) -> Vec<u8>
where
    T: TrivialType,
{
    let mut metadata = vec![ContractMetadataKind::UpdateStateless as u8];
    metadata.push(u8::try_from(method_name.len()).unwrap());
    metadata.extend_from_slice(method_name.as_bytes());
    // Number of arguments
    metadata.push(1);
    metadata.push(ContractMetadataKind::Input as u8);
    metadata.push(u8::try_from(argument_name.len()).unwrap());
    metadata.extend_from_slice(argument_name.as_bytes());
    metadata.extend_from_slice(T::METADATA);
    metadata
}

/// Fingerprint of a method created with [`update_method_metadata()`]
fn fingerprint<T>(method_name: &str, argument_name: &str) -> MethodFingerprint
where
    T: TrivialType,
{
    MethodFingerprint::new(&update_method_metadata::<T>(method_name, argument_name)).unwrap()
}

#[test]
fn named_and_tuple_forms_have_same_fingerprint() {
    // Names of arguments, types and fields don't matter
    assert_eq!(
        fingerprint::<Named>("method", "named"),
        fingerprint::<Tuple>("method", "tuple")
    );
    assert_eq!(
        fingerprint::<[Named; 3]>("method", "named"),
        fingerprint::<[Tuple; 3]>("method", "tuple")
    );
    assert_eq!(
        fingerprint::<EnumOfNamed>("method", "named"),
        fingerprint::<EnumOfTuple>("method", "tuple")
    );

    // Method names and types do
    assert_ne!(
        fingerprint::<Named>("method", "named"),
        fingerprint::<Named>("other_method", "named")
    );
    assert_ne!(
        fingerprint::<Named>("method", "named"),
        fingerprint::<[Named; 3]>("method", "named")
    );
}
