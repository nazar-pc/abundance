extern crate alloc;

use crate::registers::{EReg, ERegisters, GenericRegister, GenericRegisters, Reg, Registers};
use alloc::format;

#[test]
fn test_reg_from_bits() {
    {
        // Valid registers x0-x31
        assert_eq!(Reg::from_bits(0), Some(Reg::Zero));
        assert_eq!(Reg::from_bits(1), Some(Reg::Ra));
        assert_eq!(Reg::from_bits(2), Some(Reg::Sp));
        assert_eq!(Reg::from_bits(3), Some(Reg::Gp));
        assert_eq!(Reg::from_bits(4), Some(Reg::Tp));
        assert_eq!(Reg::from_bits(5), Some(Reg::T0));
        assert_eq!(Reg::from_bits(6), Some(Reg::T1));
        assert_eq!(Reg::from_bits(7), Some(Reg::T2));
        assert_eq!(Reg::from_bits(8), Some(Reg::S0));
        assert_eq!(Reg::from_bits(9), Some(Reg::S1));
        assert_eq!(Reg::from_bits(10), Some(Reg::A0));
        assert_eq!(Reg::from_bits(11), Some(Reg::A1));
        assert_eq!(Reg::from_bits(12), Some(Reg::A2));
        assert_eq!(Reg::from_bits(13), Some(Reg::A3));
        assert_eq!(Reg::from_bits(14), Some(Reg::A4));
        assert_eq!(Reg::from_bits(15), Some(Reg::A5));
        assert_eq!(Reg::from_bits(16), Some(Reg::A6));
        assert_eq!(Reg::from_bits(17), Some(Reg::A7));
        assert_eq!(Reg::from_bits(18), Some(Reg::S2));
        assert_eq!(Reg::from_bits(19), Some(Reg::S3));
        assert_eq!(Reg::from_bits(20), Some(Reg::S4));
        assert_eq!(Reg::from_bits(21), Some(Reg::S5));
        assert_eq!(Reg::from_bits(22), Some(Reg::S6));
        assert_eq!(Reg::from_bits(23), Some(Reg::S7));
        assert_eq!(Reg::from_bits(24), Some(Reg::S8));
        assert_eq!(Reg::from_bits(25), Some(Reg::S9));
        assert_eq!(Reg::from_bits(26), Some(Reg::S10));
        assert_eq!(Reg::from_bits(27), Some(Reg::S11));
        assert_eq!(Reg::from_bits(28), Some(Reg::T3));
        assert_eq!(Reg::from_bits(29), Some(Reg::T4));
        assert_eq!(Reg::from_bits(30), Some(Reg::T5));
        assert_eq!(Reg::from_bits(31), Some(Reg::T6));
    }

    {
        // Invalid registers
        assert_eq!(Reg::from_bits(32), None);
        assert_eq!(Reg::from_bits(64), None);
        assert_eq!(Reg::from_bits(255), None);
    }
}

#[test]
fn test_reg_display() {
    assert_eq!(format!("{}", Reg::Zero), "zero");
    assert_eq!(format!("{}", Reg::Ra), "ra");
    assert_eq!(format!("{}", Reg::Sp), "sp");
    assert_eq!(format!("{}", Reg::Gp), "gp");
    assert_eq!(format!("{}", Reg::Tp), "tp");
    assert_eq!(format!("{}", Reg::T0), "t0");
    assert_eq!(format!("{}", Reg::T1), "t1");
    assert_eq!(format!("{}", Reg::T2), "t2");
    assert_eq!(format!("{}", Reg::S0), "s0");
    assert_eq!(format!("{}", Reg::S1), "s1");
    assert_eq!(format!("{}", Reg::A0), "a0");
    assert_eq!(format!("{}", Reg::A1), "a1");
    assert_eq!(format!("{}", Reg::A2), "a2");
    assert_eq!(format!("{}", Reg::A3), "a3");
    assert_eq!(format!("{}", Reg::A4), "a4");
    assert_eq!(format!("{}", Reg::A5), "a5");
    assert_eq!(format!("{}", Reg::A6), "a6");
    assert_eq!(format!("{}", Reg::A7), "a7");
    assert_eq!(format!("{}", Reg::S2), "s2");
    assert_eq!(format!("{}", Reg::S3), "s3");
    assert_eq!(format!("{}", Reg::S4), "s4");
    assert_eq!(format!("{}", Reg::S5), "s5");
    assert_eq!(format!("{}", Reg::S6), "s6");
    assert_eq!(format!("{}", Reg::S7), "s7");
    assert_eq!(format!("{}", Reg::S8), "s8");
    assert_eq!(format!("{}", Reg::S9), "s9");
    assert_eq!(format!("{}", Reg::S10), "s10");
    assert_eq!(format!("{}", Reg::S11), "s11");
    assert_eq!(format!("{}", Reg::T3), "t3");
    assert_eq!(format!("{}", Reg::T4), "t4");
    assert_eq!(format!("{}", Reg::T5), "t5");
    assert_eq!(format!("{}", Reg::T6), "t6");
}

#[test]
fn test_reg_repr() {
    // Verify enum discriminants match expected values
    assert_eq!(Reg::Zero as u8, 0);
    assert_eq!(Reg::Ra as u8, 1);
    assert_eq!(Reg::Sp as u8, 2);
    assert_eq!(Reg::A0 as u8, 10);
    assert_eq!(Reg::A7 as u8, 17);
    assert_eq!(Reg::T6 as u8, 31);
}

#[test]
fn test_ereg_from_bits() {
    {
        // Valid registers x0-x15
        assert_eq!(EReg::from_bits(0), Some(EReg::Zero));
        assert_eq!(EReg::from_bits(1), Some(EReg::Ra));
        assert_eq!(EReg::from_bits(2), Some(EReg::Sp));
        assert_eq!(EReg::from_bits(3), Some(EReg::Gp));
        assert_eq!(EReg::from_bits(4), Some(EReg::Tp));
        assert_eq!(EReg::from_bits(5), Some(EReg::T0));
        assert_eq!(EReg::from_bits(6), Some(EReg::T1));
        assert_eq!(EReg::from_bits(7), Some(EReg::T2));
        assert_eq!(EReg::from_bits(8), Some(EReg::S0));
        assert_eq!(EReg::from_bits(9), Some(EReg::S1));
        assert_eq!(EReg::from_bits(10), Some(EReg::A0));
        assert_eq!(EReg::from_bits(11), Some(EReg::A1));
        assert_eq!(EReg::from_bits(12), Some(EReg::A2));
        assert_eq!(EReg::from_bits(13), Some(EReg::A3));
        assert_eq!(EReg::from_bits(14), Some(EReg::A4));
        assert_eq!(EReg::from_bits(15), Some(EReg::A5));
    }

    {
        // Invalid registers 16+
        assert_eq!(EReg::from_bits(16), None);
        assert_eq!(EReg::from_bits(17), None);
        assert_eq!(EReg::from_bits(31), None);
        assert_eq!(EReg::from_bits(32), None);
        assert_eq!(EReg::from_bits(255), None);
    }
}

#[test]
fn test_ereg_display() {
    assert_eq!(format!("{}", EReg::Zero), "zero");
    assert_eq!(format!("{}", EReg::Ra), "ra");
    assert_eq!(format!("{}", EReg::Sp), "sp");
    assert_eq!(format!("{}", EReg::Gp), "gp");
    assert_eq!(format!("{}", EReg::Tp), "tp");
    assert_eq!(format!("{}", EReg::T0), "t0");
    assert_eq!(format!("{}", EReg::T1), "t1");
    assert_eq!(format!("{}", EReg::T2), "t2");
    assert_eq!(format!("{}", EReg::S0), "s0");
    assert_eq!(format!("{}", EReg::S1), "s1");
    assert_eq!(format!("{}", EReg::A0), "a0");
    assert_eq!(format!("{}", EReg::A1), "a1");
    assert_eq!(format!("{}", EReg::A2), "a2");
    assert_eq!(format!("{}", EReg::A3), "a3");
    assert_eq!(format!("{}", EReg::A4), "a4");
    assert_eq!(format!("{}", EReg::A5), "a5");
}

#[test]
fn test_ereg_repr() {
    // Verify enum discriminants match expected values
    assert_eq!(EReg::Zero as u8, 0);
    assert_eq!(EReg::Ra as u8, 1);
    assert_eq!(EReg::Sp as u8, 2);
    assert_eq!(EReg::A0 as u8, 10);
    assert_eq!(EReg::A5 as u8, 15);
}

#[test]
fn test_ereg_to_reg_conversion() {
    // Test conversion from EReg to Reg
    assert_eq!(Reg::from(EReg::Zero), Reg::Zero);
    assert_eq!(Reg::from(EReg::Ra), Reg::Ra);
    assert_eq!(Reg::from(EReg::Sp), Reg::Sp);
    assert_eq!(Reg::from(EReg::Gp), Reg::Gp);
    assert_eq!(Reg::from(EReg::Tp), Reg::Tp);
    assert_eq!(Reg::from(EReg::T0), Reg::T0);
    assert_eq!(Reg::from(EReg::T1), Reg::T1);
    assert_eq!(Reg::from(EReg::T2), Reg::T2);
    assert_eq!(Reg::from(EReg::S0), Reg::S0);
    assert_eq!(Reg::from(EReg::S1), Reg::S1);
    assert_eq!(Reg::from(EReg::A0), Reg::A0);
    assert_eq!(Reg::from(EReg::A1), Reg::A1);
    assert_eq!(Reg::from(EReg::A2), Reg::A2);
    assert_eq!(Reg::from(EReg::A3), Reg::A3);
    assert_eq!(Reg::from(EReg::A4), Reg::A4);
    assert_eq!(Reg::from(EReg::A5), Reg::A5);
}

#[test]
fn test_registers_read_write() {
    {
        // Basic read/write
        let mut regs = Registers::default();
        regs.write(Reg::A0, 0xdeadbeef);
        assert_eq!(regs.read(Reg::A0), 0xdeadbeef);
    }

    {
        // Write to multiple registers
        let mut regs = Registers::default();
        regs.write(Reg::A0, 100);
        regs.write(Reg::A1, 200);
        regs.write(Reg::T0, 300);

        assert_eq!(regs.read(Reg::A0), 100);
        assert_eq!(regs.read(Reg::A1), 200);
        assert_eq!(regs.read(Reg::T0), 300);
    }

    {
        // Overwrite register
        let mut regs = Registers::default();
        regs.write(Reg::A0, 100);
        regs.write(Reg::A0, 200);
        assert_eq!(regs.read(Reg::A0), 200);
    }

    {
        // Full 64-bit values
        let mut regs = Registers::default();
        regs.write(Reg::A0, u64::MAX);
        assert_eq!(regs.read(Reg::A0), u64::MAX);

        regs.write(Reg::A1, 0x0123456789abcdef);
        assert_eq!(regs.read(Reg::A1), 0x0123456789abcdef);
    }
}

#[test]
fn test_registers_zero_register() {
    {
        // Zero register always reads 0
        let regs = Registers::default();
        assert_eq!(regs.read(Reg::Zero), 0);
    }

    {
        // Writes to zero register are ignored
        let mut regs = Registers::default();
        regs.write(Reg::Zero, 0xdeadbeef);
        assert_eq!(regs.read(Reg::Zero), 0);
    }

    {
        // Multiple writes to zero register
        let mut regs = Registers::default();
        regs.write(Reg::Zero, 100);
        regs.write(Reg::Zero, 200);
        regs.write(Reg::Zero, u64::MAX);
        assert_eq!(regs.read(Reg::Zero), 0);
    }
}

#[test]
fn test_registers_all_registers() {
    // Test all 32 registers can be written and read independently
    let mut regs = Registers::default();

    for i in 1..32 {
        let reg = Reg::from_bits(i).unwrap();
        regs.write(reg, i as u64 * 1000);
    }

    for i in 1..32 {
        let reg = Reg::from_bits(i).unwrap();
        assert_eq!(regs.read(reg), i as u64 * 1000, "Register {} failed", i);
    }

    // Zero should still be zero
    assert_eq!(regs.read(Reg::Zero), 0);
}

#[test]
fn test_eregisters_read_write() {
    {
        // Basic read/write
        let mut regs = ERegisters::default();
        regs.write(EReg::A0, 0xdeadbeef);
        assert_eq!(regs.read(EReg::A0), 0xdeadbeef);
    }

    {
        // Write to multiple registers
        let mut regs = ERegisters::default();
        regs.write(EReg::A0, 100);
        regs.write(EReg::A1, 200);
        regs.write(EReg::T0, 300);

        assert_eq!(regs.read(EReg::A0), 100);
        assert_eq!(regs.read(EReg::A1), 200);
        assert_eq!(regs.read(EReg::T0), 300);
    }

    {
        // Overwrite register
        let mut regs = ERegisters::default();
        regs.write(EReg::A0, 100);
        regs.write(EReg::A0, 200);
        assert_eq!(regs.read(EReg::A0), 200);
    }

    {
        // Full 64-bit values
        let mut regs = ERegisters::default();
        regs.write(EReg::A0, u64::MAX);
        assert_eq!(regs.read(EReg::A0), u64::MAX);

        regs.write(EReg::A1, 0x0123456789abcdef);
        assert_eq!(regs.read(EReg::A1), 0x0123456789abcdef);
    }
}

#[test]
fn test_eregisters_zero_register() {
    {
        // Zero register always reads 0
        let regs = ERegisters::default();
        assert_eq!(regs.read(EReg::Zero), 0);
    }

    {
        // Writes to zero register are ignored
        let mut regs = ERegisters::default();
        regs.write(EReg::Zero, 0xdeadbeef);
        assert_eq!(regs.read(EReg::Zero), 0);
    }

    {
        // Multiple writes to zero register
        let mut regs = ERegisters::default();
        regs.write(EReg::Zero, 100);
        regs.write(EReg::Zero, 200);
        regs.write(EReg::Zero, u64::MAX);
        assert_eq!(regs.read(EReg::Zero), 0);
    }
}

#[test]
fn test_eregisters_all_registers() {
    // Test all 16 registers can be written and read independently
    let mut regs = ERegisters::default();

    for i in 1..16 {
        let reg = EReg::from_bits(i).unwrap();
        regs.write(reg, i as u64 * 1000);
    }

    for i in 1..16 {
        let reg = EReg::from_bits(i).unwrap();
        assert_eq!(regs.read(reg), i as u64 * 1000, "Register {} failed", i);
    }

    // Zero should still be zero
    assert_eq!(regs.read(EReg::Zero), 0);
}
