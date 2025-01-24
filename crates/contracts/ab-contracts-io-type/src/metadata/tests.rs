use crate::metadata::ContractMetadataKind;

#[test]
fn check_repr() {
    let known_variants = [
        (ContractMetadataKind::Contract, 0),
        (ContractMetadataKind::Trait, 1),
        (ContractMetadataKind::Init, 2),
        (ContractMetadataKind::UpdateStateless, 3),
        (ContractMetadataKind::UpdateStatefulRo, 4),
        (ContractMetadataKind::UpdateStatefulRw, 5),
        (ContractMetadataKind::ViewStateless, 6),
        (ContractMetadataKind::ViewStatefulRo, 7),
        (ContractMetadataKind::EnvRo, 8),
        (ContractMetadataKind::EnvRw, 9),
        (ContractMetadataKind::TmpRo, 10),
        (ContractMetadataKind::TmpRw, 11),
        (ContractMetadataKind::SlotWithAddressRo, 12),
        (ContractMetadataKind::SlotWithAddressRw, 13),
        (ContractMetadataKind::SlotWithoutAddressRo, 14),
        (ContractMetadataKind::SlotWithoutAddressRw, 15),
        (ContractMetadataKind::Input, 16),
        (ContractMetadataKind::Output, 17),
        (ContractMetadataKind::Result, 18),
    ];

    for (kind, repr_byte) in known_variants {
        assert_eq!(kind as u8, repr_byte);
        assert_eq!(ContractMetadataKind::try_from_u8(repr_byte), Some(kind));
    }

    for byte in known_variants.len() as u8..=u8::MAX {
        assert_eq!(ContractMetadataKind::try_from_u8(byte), None);
    }
}
