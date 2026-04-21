extern crate alloc;

use crate::registers::general_purpose::{EReg, Reg, Register};
use alloc::format;

#[test]
fn test_reg_from_bits() {
    {
        // Valid registers x0-x31
        assert_eq!(Reg::<u64>::from_bits(0), Some(Reg::Zero));
        assert_eq!(Reg::<u64>::from_bits(1), Some(Reg::Ra));
        assert_eq!(Reg::<u64>::from_bits(2), Some(Reg::Sp));
        assert_eq!(Reg::<u64>::from_bits(3), Some(Reg::Gp));
        assert_eq!(Reg::<u64>::from_bits(4), Some(Reg::Tp));
        assert_eq!(Reg::<u64>::from_bits(5), Some(Reg::T0));
        assert_eq!(Reg::<u64>::from_bits(6), Some(Reg::T1));
        assert_eq!(Reg::<u64>::from_bits(7), Some(Reg::T2));
        assert_eq!(Reg::<u64>::from_bits(8), Some(Reg::S0));
        assert_eq!(Reg::<u64>::from_bits(9), Some(Reg::S1));
        assert_eq!(Reg::<u64>::from_bits(10), Some(Reg::A0));
        assert_eq!(Reg::<u64>::from_bits(11), Some(Reg::A1));
        assert_eq!(Reg::<u64>::from_bits(12), Some(Reg::A2));
        assert_eq!(Reg::<u64>::from_bits(13), Some(Reg::A3));
        assert_eq!(Reg::<u64>::from_bits(14), Some(Reg::A4));
        assert_eq!(Reg::<u64>::from_bits(15), Some(Reg::A5));
        assert_eq!(Reg::<u64>::from_bits(16), Some(Reg::A6));
        assert_eq!(Reg::<u64>::from_bits(17), Some(Reg::A7));
        assert_eq!(Reg::<u64>::from_bits(18), Some(Reg::S2));
        assert_eq!(Reg::<u64>::from_bits(19), Some(Reg::S3));
        assert_eq!(Reg::<u64>::from_bits(20), Some(Reg::S4));
        assert_eq!(Reg::<u64>::from_bits(21), Some(Reg::S5));
        assert_eq!(Reg::<u64>::from_bits(22), Some(Reg::S6));
        assert_eq!(Reg::<u64>::from_bits(23), Some(Reg::S7));
        assert_eq!(Reg::<u64>::from_bits(24), Some(Reg::S8));
        assert_eq!(Reg::<u64>::from_bits(25), Some(Reg::S9));
        assert_eq!(Reg::<u64>::from_bits(26), Some(Reg::S10));
        assert_eq!(Reg::<u64>::from_bits(27), Some(Reg::S11));
        assert_eq!(Reg::<u64>::from_bits(28), Some(Reg::T3));
        assert_eq!(Reg::<u64>::from_bits(29), Some(Reg::T4));
        assert_eq!(Reg::<u64>::from_bits(30), Some(Reg::T5));
        assert_eq!(Reg::<u64>::from_bits(31), Some(Reg::T6));
    }

    {
        // Invalid registers
        assert_eq!(Reg::<u64>::from_bits(32), None);
        assert_eq!(Reg::<u64>::from_bits(64), None);
        assert_eq!(Reg::<u64>::from_bits(255), None);
    }
}

#[test]
fn test_reg_display() {
    assert_eq!(format!("{}", Reg::<u64>::Zero), "zero");
    assert_eq!(format!("{}", Reg::<u64>::Ra), "ra");
    assert_eq!(format!("{}", Reg::<u64>::Sp), "sp");
    assert_eq!(format!("{}", Reg::<u64>::Gp), "gp");
    assert_eq!(format!("{}", Reg::<u64>::Tp), "tp");
    assert_eq!(format!("{}", Reg::<u64>::T0), "t0");
    assert_eq!(format!("{}", Reg::<u64>::T1), "t1");
    assert_eq!(format!("{}", Reg::<u64>::T2), "t2");
    assert_eq!(format!("{}", Reg::<u64>::S0), "s0");
    assert_eq!(format!("{}", Reg::<u64>::S1), "s1");
    assert_eq!(format!("{}", Reg::<u64>::A0), "a0");
    assert_eq!(format!("{}", Reg::<u64>::A1), "a1");
    assert_eq!(format!("{}", Reg::<u64>::A2), "a2");
    assert_eq!(format!("{}", Reg::<u64>::A3), "a3");
    assert_eq!(format!("{}", Reg::<u64>::A4), "a4");
    assert_eq!(format!("{}", Reg::<u64>::A5), "a5");
    assert_eq!(format!("{}", Reg::<u64>::A6), "a6");
    assert_eq!(format!("{}", Reg::<u64>::A7), "a7");
    assert_eq!(format!("{}", Reg::<u64>::S2), "s2");
    assert_eq!(format!("{}", Reg::<u64>::S3), "s3");
    assert_eq!(format!("{}", Reg::<u64>::S4), "s4");
    assert_eq!(format!("{}", Reg::<u64>::S5), "s5");
    assert_eq!(format!("{}", Reg::<u64>::S6), "s6");
    assert_eq!(format!("{}", Reg::<u64>::S7), "s7");
    assert_eq!(format!("{}", Reg::<u64>::S8), "s8");
    assert_eq!(format!("{}", Reg::<u64>::S9), "s9");
    assert_eq!(format!("{}", Reg::<u64>::S10), "s10");
    assert_eq!(format!("{}", Reg::<u64>::S11), "s11");
    assert_eq!(format!("{}", Reg::<u64>::T3), "t3");
    assert_eq!(format!("{}", Reg::<u64>::T4), "t4");
    assert_eq!(format!("{}", Reg::<u64>::T5), "t5");
    assert_eq!(format!("{}", Reg::<u64>::T6), "t6");
}

#[test]
fn test_ereg_from_bits() {
    {
        // Valid registers x0-x15
        assert_eq!(EReg::<u64>::from_bits(0), Some(EReg::Zero));
        assert_eq!(EReg::<u64>::from_bits(1), Some(EReg::Ra));
        assert_eq!(EReg::<u64>::from_bits(2), Some(EReg::Sp));
        assert_eq!(EReg::<u64>::from_bits(3), Some(EReg::Gp));
        assert_eq!(EReg::<u64>::from_bits(4), Some(EReg::Tp));
        assert_eq!(EReg::<u64>::from_bits(5), Some(EReg::T0));
        assert_eq!(EReg::<u64>::from_bits(6), Some(EReg::T1));
        assert_eq!(EReg::<u64>::from_bits(7), Some(EReg::T2));
        assert_eq!(EReg::<u64>::from_bits(8), Some(EReg::S0));
        assert_eq!(EReg::<u64>::from_bits(9), Some(EReg::S1));
        assert_eq!(EReg::<u64>::from_bits(10), Some(EReg::A0));
        assert_eq!(EReg::<u64>::from_bits(11), Some(EReg::A1));
        assert_eq!(EReg::<u64>::from_bits(12), Some(EReg::A2));
        assert_eq!(EReg::<u64>::from_bits(13), Some(EReg::A3));
        assert_eq!(EReg::<u64>::from_bits(14), Some(EReg::A4));
        assert_eq!(EReg::<u64>::from_bits(15), Some(EReg::A5));
    }

    {
        // Invalid registers 16+
        assert_eq!(EReg::<u64>::from_bits(16), None);
        assert_eq!(EReg::<u64>::from_bits(17), None);
        assert_eq!(EReg::<u64>::from_bits(31), None);
        assert_eq!(EReg::<u64>::from_bits(32), None);
        assert_eq!(EReg::<u64>::from_bits(255), None);
    }
}

#[test]
fn test_ereg_display() {
    assert_eq!(format!("{}", EReg::<u64>::Zero), "zero");
    assert_eq!(format!("{}", EReg::<u64>::Ra), "ra");
    assert_eq!(format!("{}", EReg::<u64>::Sp), "sp");
    assert_eq!(format!("{}", EReg::<u64>::Gp), "gp");
    assert_eq!(format!("{}", EReg::<u64>::Tp), "tp");
    assert_eq!(format!("{}", EReg::<u64>::T0), "t0");
    assert_eq!(format!("{}", EReg::<u64>::T1), "t1");
    assert_eq!(format!("{}", EReg::<u64>::T2), "t2");
    assert_eq!(format!("{}", EReg::<u64>::S0), "s0");
    assert_eq!(format!("{}", EReg::<u64>::S1), "s1");
    assert_eq!(format!("{}", EReg::<u64>::A0), "a0");
    assert_eq!(format!("{}", EReg::<u64>::A1), "a1");
    assert_eq!(format!("{}", EReg::<u64>::A2), "a2");
    assert_eq!(format!("{}", EReg::<u64>::A3), "a3");
    assert_eq!(format!("{}", EReg::<u64>::A4), "a4");
    assert_eq!(format!("{}", EReg::<u64>::A5), "a5");
}

#[test]
fn test_ereg_to_reg_conversion() {
    // Test conversion from EReg to Reg
    assert_eq!(Reg::<u64>::from(EReg::<u64>::Zero), Reg::Zero);
    assert_eq!(Reg::<u64>::from(EReg::<u64>::Ra), Reg::Ra);
    assert_eq!(Reg::<u64>::from(EReg::<u64>::Sp), Reg::Sp);
    assert_eq!(Reg::<u64>::from(EReg::<u64>::Gp), Reg::Gp);
    assert_eq!(Reg::<u64>::from(EReg::<u64>::Tp), Reg::Tp);
    assert_eq!(Reg::<u64>::from(EReg::<u64>::T0), Reg::T0);
    assert_eq!(Reg::<u64>::from(EReg::<u64>::T1), Reg::T1);
    assert_eq!(Reg::<u64>::from(EReg::<u64>::T2), Reg::T2);
    assert_eq!(Reg::<u64>::from(EReg::<u64>::S0), Reg::S0);
    assert_eq!(Reg::<u64>::from(EReg::<u64>::S1), Reg::S1);
    assert_eq!(Reg::<u64>::from(EReg::<u64>::A0), Reg::A0);
    assert_eq!(Reg::<u64>::from(EReg::<u64>::A1), Reg::A1);
    assert_eq!(Reg::<u64>::from(EReg::<u64>::A2), Reg::A2);
    assert_eq!(Reg::<u64>::from(EReg::<u64>::A3), Reg::A3);
    assert_eq!(Reg::<u64>::from(EReg::<u64>::A4), Reg::A4);
    assert_eq!(Reg::<u64>::from(EReg::<u64>::A5), Reg::A5);
}
