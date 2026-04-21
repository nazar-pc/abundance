use crate::basic::BasicRegisters;
use ab_riscv_primitives::prelude::*;

#[test]
fn test_registers_read_write() {
    {
        // Basic read/write
        let mut regs = BasicRegisters::<Reg<u64>>::default();
        regs.write(Reg::A0, 0xdeadbeef);
        assert_eq!(regs.read(Reg::A0), 0xdeadbeef);
    }

    {
        // Write to multiple registers
        let mut regs = BasicRegisters::<Reg<u64>>::default();
        regs.write(Reg::A0, 100);
        regs.write(Reg::A1, 200);
        regs.write(Reg::T0, 300);

        assert_eq!(regs.read(Reg::A0), 100);
        assert_eq!(regs.read(Reg::A1), 200);
        assert_eq!(regs.read(Reg::T0), 300);
    }

    {
        // Overwrite register
        let mut regs = BasicRegisters::<Reg<u64>>::default();
        regs.write(Reg::A0, 100);
        regs.write(Reg::A0, 200);
        assert_eq!(regs.read(Reg::A0), 200);
    }

    {
        // Full 64-bit values
        let mut regs = BasicRegisters::<Reg<u64>>::default();
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
        let regs = BasicRegisters::<Reg<u64>>::default();
        assert_eq!(regs.read(Reg::Zero), 0);
    }

    {
        // Writes to zero register are ignored
        let mut regs = BasicRegisters::<Reg<u64>>::default();
        regs.write(Reg::Zero, 0xdeadbeef);
        assert_eq!(regs.read(Reg::Zero), 0);
    }

    {
        // Multiple writes to zero register
        let mut regs = BasicRegisters::<Reg<u64>>::default();
        regs.write(Reg::Zero, 100);
        regs.write(Reg::Zero, 200);
        regs.write(Reg::Zero, u64::MAX);
        assert_eq!(regs.read(Reg::Zero), 0);
    }
}

#[test]
fn test_registers_all_registers() {
    // Test all 32 registers can be written and read independently
    let mut regs = BasicRegisters::<Reg<u64>>::default();

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
        let mut regs = BasicRegisters::default();
        regs.write(EReg::<u64>::A0, 0xdeadbeef);
        assert_eq!(regs.read(EReg::<u64>::A0), 0xdeadbeef);
    }

    {
        // Write to multiple registers
        let mut regs = BasicRegisters::default();
        regs.write(EReg::<u64>::A0, 100);
        regs.write(EReg::<u64>::A1, 200);
        regs.write(EReg::<u64>::T0, 300);

        assert_eq!(regs.read(EReg::<u64>::A0), 100);
        assert_eq!(regs.read(EReg::<u64>::A1), 200);
        assert_eq!(regs.read(EReg::<u64>::T0), 300);
    }

    {
        // Overwrite register
        let mut regs = BasicRegisters::default();
        regs.write(EReg::<u64>::A0, 100);
        regs.write(EReg::<u64>::A0, 200);
        assert_eq!(regs.read(EReg::<u64>::A0), 200);
    }

    {
        // Full 64-bit values
        let mut regs = BasicRegisters::default();
        regs.write(EReg::<u64>::A0, u64::MAX);
        assert_eq!(regs.read(EReg::<u64>::A0), u64::MAX);

        regs.write(EReg::<u64>::A1, 0x0123456789abcdef);
        assert_eq!(regs.read(EReg::<u64>::A1), 0x0123456789abcdef);
    }
}

#[test]
fn test_eregisters_zero_register() {
    {
        // Zero register always reads 0
        let regs = BasicRegisters::default();
        assert_eq!(regs.read(EReg::<u64>::Zero), 0);
    }

    {
        // Writes to zero register are ignored
        let mut regs = BasicRegisters::default();
        regs.write(EReg::<u64>::Zero, 0xdeadbeef);
        assert_eq!(regs.read(EReg::<u64>::Zero), 0);
    }

    {
        // Multiple writes to zero register
        let mut regs = BasicRegisters::default();
        regs.write(EReg::<u64>::Zero, 100);
        regs.write(EReg::<u64>::Zero, 200);
        regs.write(EReg::<u64>::Zero, u64::MAX);
        assert_eq!(regs.read(EReg::<u64>::Zero), 0);
    }
}

#[test]
fn test_eregisters_all_registers() {
    // Test all 16 registers can be written and read independently
    let mut regs = BasicRegisters::default();

    for i in 1..16 {
        let reg = EReg::<u64>::from_bits(i).unwrap();
        regs.write(reg, i as u64 * 1000);
    }

    for i in 1..16 {
        let reg = EReg::<u64>::from_bits(i).unwrap();
        assert_eq!(regs.read(reg), i as u64 * 1000, "Register {} failed", i);
    }

    // Zero should still be zero
    assert_eq!(regs.read(EReg::<u64>::Zero), 0);
}
