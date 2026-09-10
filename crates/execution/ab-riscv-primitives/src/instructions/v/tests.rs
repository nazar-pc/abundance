use crate::instructions::v::{VRegGroupSize, Vlmul};

#[test]
fn widening_register_count_values() {
    // EMUL = 2 * LMUL:
    // Mf8 (1/8) -> 2/8 = 1/4 -> 1 reg
    // Mf4 (1/4) -> 2/4 = 1/2 -> 1 reg
    // Mf2 (1/2) -> 2/2 = 1   -> 1 reg
    // M1 (1)    -> 2/1 = 2   -> 2 regs
    // M2 (2)    -> 4/1 = 4   -> 4 regs
    // M4 (4)    -> 8/1 = 8   -> 8 regs
    // M8 (8)    -> 16/1 = 16 -> None (illegal)
    assert_eq!(
        Vlmul::widening_register_count(Vlmul::Mf8),
        Some(VRegGroupSize::R1)
    );
    assert_eq!(
        Vlmul::widening_register_count(Vlmul::Mf4),
        Some(VRegGroupSize::R1)
    );
    assert_eq!(
        Vlmul::widening_register_count(Vlmul::Mf2),
        Some(VRegGroupSize::R1)
    );
    assert_eq!(
        Vlmul::widening_register_count(Vlmul::M1),
        Some(VRegGroupSize::R2)
    );
    assert_eq!(
        Vlmul::widening_register_count(Vlmul::M2),
        Some(VRegGroupSize::R4)
    );
    assert_eq!(
        Vlmul::widening_register_count(Vlmul::M4),
        Some(VRegGroupSize::R8)
    );
    assert_eq!(Vlmul::widening_register_count(Vlmul::M8), None);
}
