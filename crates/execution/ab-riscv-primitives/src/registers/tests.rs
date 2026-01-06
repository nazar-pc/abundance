extern crate alloc;

use crate::registers::{
    EReg64, ERegisters64, GenericRegister64, GenericRegisters64, Reg64, Registers64,
};
use alloc::format;

#[test]
fn test_reg_from_bits() {
    {
        // Valid registers x0-x31
        assert_eq!(Reg64::from_bits(0), Some(Reg64::Zero));
        assert_eq!(Reg64::from_bits(1), Some(Reg64::Ra));
        assert_eq!(Reg64::from_bits(2), Some(Reg64::Sp));
        assert_eq!(Reg64::from_bits(3), Some(Reg64::Gp));
        assert_eq!(Reg64::from_bits(4), Some(Reg64::Tp));
        assert_eq!(Reg64::from_bits(5), Some(Reg64::T0));
        assert_eq!(Reg64::from_bits(6), Some(Reg64::T1));
        assert_eq!(Reg64::from_bits(7), Some(Reg64::T2));
        assert_eq!(Reg64::from_bits(8), Some(Reg64::S0));
        assert_eq!(Reg64::from_bits(9), Some(Reg64::S1));
        assert_eq!(Reg64::from_bits(10), Some(Reg64::A0));
        assert_eq!(Reg64::from_bits(11), Some(Reg64::A1));
        assert_eq!(Reg64::from_bits(12), Some(Reg64::A2));
        assert_eq!(Reg64::from_bits(13), Some(Reg64::A3));
        assert_eq!(Reg64::from_bits(14), Some(Reg64::A4));
        assert_eq!(Reg64::from_bits(15), Some(Reg64::A5));
        assert_eq!(Reg64::from_bits(16), Some(Reg64::A6));
        assert_eq!(Reg64::from_bits(17), Some(Reg64::A7));
        assert_eq!(Reg64::from_bits(18), Some(Reg64::S2));
        assert_eq!(Reg64::from_bits(19), Some(Reg64::S3));
        assert_eq!(Reg64::from_bits(20), Some(Reg64::S4));
        assert_eq!(Reg64::from_bits(21), Some(Reg64::S5));
        assert_eq!(Reg64::from_bits(22), Some(Reg64::S6));
        assert_eq!(Reg64::from_bits(23), Some(Reg64::S7));
        assert_eq!(Reg64::from_bits(24), Some(Reg64::S8));
        assert_eq!(Reg64::from_bits(25), Some(Reg64::S9));
        assert_eq!(Reg64::from_bits(26), Some(Reg64::S10));
        assert_eq!(Reg64::from_bits(27), Some(Reg64::S11));
        assert_eq!(Reg64::from_bits(28), Some(Reg64::T3));
        assert_eq!(Reg64::from_bits(29), Some(Reg64::T4));
        assert_eq!(Reg64::from_bits(30), Some(Reg64::T5));
        assert_eq!(Reg64::from_bits(31), Some(Reg64::T6));
    }

    {
        // Invalid registers
        assert_eq!(Reg64::from_bits(32), None);
        assert_eq!(Reg64::from_bits(64), None);
        assert_eq!(Reg64::from_bits(255), None);
    }
}

#[test]
fn test_reg_display() {
    assert_eq!(format!("{}", Reg64::Zero), "zero");
    assert_eq!(format!("{}", Reg64::Ra), "ra");
    assert_eq!(format!("{}", Reg64::Sp), "sp");
    assert_eq!(format!("{}", Reg64::Gp), "gp");
    assert_eq!(format!("{}", Reg64::Tp), "tp");
    assert_eq!(format!("{}", Reg64::T0), "t0");
    assert_eq!(format!("{}", Reg64::T1), "t1");
    assert_eq!(format!("{}", Reg64::T2), "t2");
    assert_eq!(format!("{}", Reg64::S0), "s0");
    assert_eq!(format!("{}", Reg64::S1), "s1");
    assert_eq!(format!("{}", Reg64::A0), "a0");
    assert_eq!(format!("{}", Reg64::A1), "a1");
    assert_eq!(format!("{}", Reg64::A2), "a2");
    assert_eq!(format!("{}", Reg64::A3), "a3");
    assert_eq!(format!("{}", Reg64::A4), "a4");
    assert_eq!(format!("{}", Reg64::A5), "a5");
    assert_eq!(format!("{}", Reg64::A6), "a6");
    assert_eq!(format!("{}", Reg64::A7), "a7");
    assert_eq!(format!("{}", Reg64::S2), "s2");
    assert_eq!(format!("{}", Reg64::S3), "s3");
    assert_eq!(format!("{}", Reg64::S4), "s4");
    assert_eq!(format!("{}", Reg64::S5), "s5");
    assert_eq!(format!("{}", Reg64::S6), "s6");
    assert_eq!(format!("{}", Reg64::S7), "s7");
    assert_eq!(format!("{}", Reg64::S8), "s8");
    assert_eq!(format!("{}", Reg64::S9), "s9");
    assert_eq!(format!("{}", Reg64::S10), "s10");
    assert_eq!(format!("{}", Reg64::S11), "s11");
    assert_eq!(format!("{}", Reg64::T3), "t3");
    assert_eq!(format!("{}", Reg64::T4), "t4");
    assert_eq!(format!("{}", Reg64::T5), "t5");
    assert_eq!(format!("{}", Reg64::T6), "t6");
}

#[test]
fn test_reg_repr() {
    // Verify enum discriminants match expected values
    assert_eq!(Reg64::Zero as u8, 0);
    assert_eq!(Reg64::Ra as u8, 1);
    assert_eq!(Reg64::Sp as u8, 2);
    assert_eq!(Reg64::A0 as u8, 10);
    assert_eq!(Reg64::A7 as u8, 17);
    assert_eq!(Reg64::T6 as u8, 31);
}

#[test]
fn test_ereg_from_bits() {
    {
        // Valid registers x0-x15
        assert_eq!(EReg64::from_bits(0), Some(EReg64::Zero));
        assert_eq!(EReg64::from_bits(1), Some(EReg64::Ra));
        assert_eq!(EReg64::from_bits(2), Some(EReg64::Sp));
        assert_eq!(EReg64::from_bits(3), Some(EReg64::Gp));
        assert_eq!(EReg64::from_bits(4), Some(EReg64::Tp));
        assert_eq!(EReg64::from_bits(5), Some(EReg64::T0));
        assert_eq!(EReg64::from_bits(6), Some(EReg64::T1));
        assert_eq!(EReg64::from_bits(7), Some(EReg64::T2));
        assert_eq!(EReg64::from_bits(8), Some(EReg64::S0));
        assert_eq!(EReg64::from_bits(9), Some(EReg64::S1));
        assert_eq!(EReg64::from_bits(10), Some(EReg64::A0));
        assert_eq!(EReg64::from_bits(11), Some(EReg64::A1));
        assert_eq!(EReg64::from_bits(12), Some(EReg64::A2));
        assert_eq!(EReg64::from_bits(13), Some(EReg64::A3));
        assert_eq!(EReg64::from_bits(14), Some(EReg64::A4));
        assert_eq!(EReg64::from_bits(15), Some(EReg64::A5));
    }

    {
        // Invalid registers 16+
        assert_eq!(EReg64::from_bits(16), None);
        assert_eq!(EReg64::from_bits(17), None);
        assert_eq!(EReg64::from_bits(31), None);
        assert_eq!(EReg64::from_bits(32), None);
        assert_eq!(EReg64::from_bits(255), None);
    }
}

#[test]
fn test_ereg_display() {
    assert_eq!(format!("{}", EReg64::Zero), "zero");
    assert_eq!(format!("{}", EReg64::Ra), "ra");
    assert_eq!(format!("{}", EReg64::Sp), "sp");
    assert_eq!(format!("{}", EReg64::Gp), "gp");
    assert_eq!(format!("{}", EReg64::Tp), "tp");
    assert_eq!(format!("{}", EReg64::T0), "t0");
    assert_eq!(format!("{}", EReg64::T1), "t1");
    assert_eq!(format!("{}", EReg64::T2), "t2");
    assert_eq!(format!("{}", EReg64::S0), "s0");
    assert_eq!(format!("{}", EReg64::S1), "s1");
    assert_eq!(format!("{}", EReg64::A0), "a0");
    assert_eq!(format!("{}", EReg64::A1), "a1");
    assert_eq!(format!("{}", EReg64::A2), "a2");
    assert_eq!(format!("{}", EReg64::A3), "a3");
    assert_eq!(format!("{}", EReg64::A4), "a4");
    assert_eq!(format!("{}", EReg64::A5), "a5");
}

#[test]
fn test_ereg_repr() {
    // Verify enum discriminants match expected values
    assert_eq!(EReg64::Zero as u8, 0);
    assert_eq!(EReg64::Ra as u8, 1);
    assert_eq!(EReg64::Sp as u8, 2);
    assert_eq!(EReg64::A0 as u8, 10);
    assert_eq!(EReg64::A5 as u8, 15);
}

#[test]
fn test_ereg_to_reg_conversion() {
    // Test conversion from EReg to Reg
    assert_eq!(Reg64::from(EReg64::Zero), Reg64::Zero);
    assert_eq!(Reg64::from(EReg64::Ra), Reg64::Ra);
    assert_eq!(Reg64::from(EReg64::Sp), Reg64::Sp);
    assert_eq!(Reg64::from(EReg64::Gp), Reg64::Gp);
    assert_eq!(Reg64::from(EReg64::Tp), Reg64::Tp);
    assert_eq!(Reg64::from(EReg64::T0), Reg64::T0);
    assert_eq!(Reg64::from(EReg64::T1), Reg64::T1);
    assert_eq!(Reg64::from(EReg64::T2), Reg64::T2);
    assert_eq!(Reg64::from(EReg64::S0), Reg64::S0);
    assert_eq!(Reg64::from(EReg64::S1), Reg64::S1);
    assert_eq!(Reg64::from(EReg64::A0), Reg64::A0);
    assert_eq!(Reg64::from(EReg64::A1), Reg64::A1);
    assert_eq!(Reg64::from(EReg64::A2), Reg64::A2);
    assert_eq!(Reg64::from(EReg64::A3), Reg64::A3);
    assert_eq!(Reg64::from(EReg64::A4), Reg64::A4);
    assert_eq!(Reg64::from(EReg64::A5), Reg64::A5);
}

#[test]
fn test_registers_read_write() {
    {
        // Basic read/write
        let mut regs = Registers64::default();
        regs.write(Reg64::A0, 0xdeadbeef);
        assert_eq!(regs.read(Reg64::A0), 0xdeadbeef);
    }

    {
        // Write to multiple registers
        let mut regs = Registers64::default();
        regs.write(Reg64::A0, 100);
        regs.write(Reg64::A1, 200);
        regs.write(Reg64::T0, 300);

        assert_eq!(regs.read(Reg64::A0), 100);
        assert_eq!(regs.read(Reg64::A1), 200);
        assert_eq!(regs.read(Reg64::T0), 300);
    }

    {
        // Overwrite register
        let mut regs = Registers64::default();
        regs.write(Reg64::A0, 100);
        regs.write(Reg64::A0, 200);
        assert_eq!(regs.read(Reg64::A0), 200);
    }

    {
        // Full 64-bit values
        let mut regs = Registers64::default();
        regs.write(Reg64::A0, u64::MAX);
        assert_eq!(regs.read(Reg64::A0), u64::MAX);

        regs.write(Reg64::A1, 0x0123456789abcdef);
        assert_eq!(regs.read(Reg64::A1), 0x0123456789abcdef);
    }
}

#[test]
fn test_registers_zero_register() {
    {
        // Zero register always reads 0
        let regs = Registers64::default();
        assert_eq!(regs.read(Reg64::Zero), 0);
    }

    {
        // Writes to zero register are ignored
        let mut regs = Registers64::default();
        regs.write(Reg64::Zero, 0xdeadbeef);
        assert_eq!(regs.read(Reg64::Zero), 0);
    }

    {
        // Multiple writes to zero register
        let mut regs = Registers64::default();
        regs.write(Reg64::Zero, 100);
        regs.write(Reg64::Zero, 200);
        regs.write(Reg64::Zero, u64::MAX);
        assert_eq!(regs.read(Reg64::Zero), 0);
    }
}

#[test]
fn test_registers_all_registers() {
    // Test all 32 registers can be written and read independently
    let mut regs = Registers64::default();

    for i in 1..32 {
        let reg = Reg64::from_bits(i).unwrap();
        regs.write(reg, i as u64 * 1000);
    }

    for i in 1..32 {
        let reg = Reg64::from_bits(i).unwrap();
        assert_eq!(regs.read(reg), i as u64 * 1000, "Register {} failed", i);
    }

    // Zero should still be zero
    assert_eq!(regs.read(Reg64::Zero), 0);
}

#[test]
fn test_eregisters_read_write() {
    {
        // Basic read/write
        let mut regs = ERegisters64::default();
        regs.write(EReg64::A0, 0xdeadbeef);
        assert_eq!(regs.read(EReg64::A0), 0xdeadbeef);
    }

    {
        // Write to multiple registers
        let mut regs = ERegisters64::default();
        regs.write(EReg64::A0, 100);
        regs.write(EReg64::A1, 200);
        regs.write(EReg64::T0, 300);

        assert_eq!(regs.read(EReg64::A0), 100);
        assert_eq!(regs.read(EReg64::A1), 200);
        assert_eq!(regs.read(EReg64::T0), 300);
    }

    {
        // Overwrite register
        let mut regs = ERegisters64::default();
        regs.write(EReg64::A0, 100);
        regs.write(EReg64::A0, 200);
        assert_eq!(regs.read(EReg64::A0), 200);
    }

    {
        // Full 64-bit values
        let mut regs = ERegisters64::default();
        regs.write(EReg64::A0, u64::MAX);
        assert_eq!(regs.read(EReg64::A0), u64::MAX);

        regs.write(EReg64::A1, 0x0123456789abcdef);
        assert_eq!(regs.read(EReg64::A1), 0x0123456789abcdef);
    }
}

#[test]
fn test_eregisters_zero_register() {
    {
        // Zero register always reads 0
        let regs = ERegisters64::default();
        assert_eq!(regs.read(EReg64::Zero), 0);
    }

    {
        // Writes to zero register are ignored
        let mut regs = ERegisters64::default();
        regs.write(EReg64::Zero, 0xdeadbeef);
        assert_eq!(regs.read(EReg64::Zero), 0);
    }

    {
        // Multiple writes to zero register
        let mut regs = ERegisters64::default();
        regs.write(EReg64::Zero, 100);
        regs.write(EReg64::Zero, 200);
        regs.write(EReg64::Zero, u64::MAX);
        assert_eq!(regs.read(EReg64::Zero), 0);
    }
}

#[test]
fn test_eregisters_all_registers() {
    // Test all 16 registers can be written and read independently
    let mut regs = ERegisters64::default();

    for i in 1..16 {
        let reg = EReg64::from_bits(i).unwrap();
        regs.write(reg, i as u64 * 1000);
    }

    for i in 1..16 {
        let reg = EReg64::from_bits(i).unwrap();
        assert_eq!(regs.read(reg), i as u64 * 1000, "Register {} failed", i);
    }

    // Zero should still be zero
    assert_eq!(regs.read(EReg64::Zero), 0);
}
