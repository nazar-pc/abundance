use crate::RegisterFile;
use crate::basic::{BasicRegister, BasicRegisters, RegisterOffset};
use ab_riscv_primitives::prelude::*;
use core::fmt;

/// Register type with two registers at both ends of the offset range, with `x0` at the last offset
#[derive(Debug, Clone, Copy)]
#[derive_const(Default, PartialEq, Eq)]
enum SparseReg {
    #[default]
    Zero,
    Other,
}

impl fmt::Display for SparseReg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

const impl Register for SparseReg {
    const RVE: bool = false;
    const ZERO: Self = Self::Zero;
    // ABI registers are irrelevant for these tests
    const SP: Self = Self::Other;
    const RA: Self = Self::Other;
    const A0: Self = Self::Other;
    const A1: Self = Self::Other;
    type Type = u64;

    fn from_bits(bits: u8) -> Option<Self> {
        match bits {
            0 => Some(Self::Zero),
            1 => Some(Self::Other),
            _ => None,
        }
    }
}

const impl BasicRegister for SparseReg {
    fn offset(self) -> RegisterOffset {
        match self {
            Self::Zero => RegisterOffset::new(31),
            Self::Other => RegisterOffset::new(0),
        }
        .expect("Both offsets are below 32; qed")
    }
}

#[test]
fn test_register_offset() {
    assert_eq!(RegisterOffset::new(0).map(usize::from), Some(0));
    assert_eq!(
        RegisterOffset::new(31).map(usize::from),
        Some(RegisterOffset::COUNT - 1)
    );
    assert!(RegisterOffset::new(32).is_none());
    assert!(RegisterOffset::new(u8::MAX).is_none());
}

#[test]
fn test_custom_registers() {
    {
        let mut regs = BasicRegisters::<SparseReg, false>::default();
        regs.write(SparseReg::Other, u64::MAX);
        regs.write(SparseReg::Zero, 1);
        assert_eq!(regs.read(SparseReg::Other), u64::MAX);
        assert_eq!(regs.read(SparseReg::Zero), 0);
    }

    {
        let mut regs = BasicRegisters::<SparseReg, true>::default();
        regs.write(SparseReg::Other, u64::MAX);
        regs.write(SparseReg::Zero, 1);
        assert_eq!(regs.read(SparseReg::Other), u64::MAX);
        assert_eq!(regs.read(SparseReg::Zero), 0);
    }
}

#[test]
fn test_registers_read_write() {
    {
        // Basic read/write
        let mut regs = BasicRegisters::<Reg<u64>>::default();
        regs.write(Reg::A0, 0xdead_beef);
        assert_eq!(regs.read(Reg::A0), 0xdead_beef);
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

        regs.write(Reg::A1, 0x0123_4567_89ab_cdef);
        assert_eq!(regs.read(Reg::A1), 0x0123_4567_89ab_cdef);
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
        regs.write(Reg::Zero, 0xdead_beef);
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
        regs.write(reg, u64::from(i) * 1000);
    }

    for i in 1..32 {
        let reg = Reg::from_bits(i).unwrap();
        assert_eq!(regs.read(reg), u64::from(i) * 1000, "Register {i} failed");
    }

    // Zero should still be zero
    assert_eq!(regs.read(Reg::Zero), 0);
}

#[test]
fn test_eregisters_read_write() {
    {
        // Basic read/write
        let mut regs = BasicRegisters::<_, false>::default();
        regs.write(EReg::<u64>::A0, 0xdead_beef);
        assert_eq!(regs.read(EReg::<u64>::A0), 0xdead_beef);
    }

    {
        // Write to multiple registers
        let mut regs = BasicRegisters::<_, false>::default();
        regs.write(EReg::<u64>::A0, 100);
        regs.write(EReg::<u64>::A1, 200);
        regs.write(EReg::<u64>::T0, 300);

        assert_eq!(regs.read(EReg::<u64>::A0), 100);
        assert_eq!(regs.read(EReg::<u64>::A1), 200);
        assert_eq!(regs.read(EReg::<u64>::T0), 300);
    }

    {
        // Overwrite register
        let mut regs = BasicRegisters::<_, false>::default();
        regs.write(EReg::<u64>::A0, 100);
        regs.write(EReg::<u64>::A0, 200);
        assert_eq!(regs.read(EReg::<u64>::A0), 200);
    }

    {
        // Full 64-bit values
        let mut regs = BasicRegisters::<_, false>::default();
        regs.write(EReg::<u64>::A0, u64::MAX);
        assert_eq!(regs.read(EReg::<u64>::A0), u64::MAX);

        regs.write(EReg::<u64>::A1, 0x0123_4567_89ab_cdef);
        assert_eq!(regs.read(EReg::<u64>::A1), 0x0123_4567_89ab_cdef);
    }
}

#[test]
fn test_eregisters_zero_register() {
    {
        // Zero register always reads 0
        let regs = BasicRegisters::<_, false>::default();
        assert_eq!(regs.read(EReg::<u64>::Zero), 0);
    }

    {
        // Writes to zero register are ignored
        let mut regs = BasicRegisters::<_, false>::default();
        regs.write(EReg::<u64>::Zero, 0xdead_beef);
        assert_eq!(regs.read(EReg::<u64>::Zero), 0);
    }

    {
        // Multiple writes to zero register
        let mut regs = BasicRegisters::<_, false>::default();
        regs.write(EReg::<u64>::Zero, 100);
        regs.write(EReg::<u64>::Zero, 200);
        regs.write(EReg::<u64>::Zero, u64::MAX);
        assert_eq!(regs.read(EReg::<u64>::Zero), 0);
    }
}

#[test]
fn test_eregisters_all_registers() {
    // Test all 16 registers can be written and read independently
    let mut regs = BasicRegisters::<_, false>::default();

    for i in 1..16 {
        let reg = EReg::<u64>::from_bits(i).unwrap();
        regs.write(reg, u64::from(i) * 1000);
    }

    for i in 1..16 {
        let reg = EReg::<u64>::from_bits(i).unwrap();
        assert_eq!(regs.read(reg), u64::from(i) * 1000, "Register {i} failed");
    }

    // Zero should still be zero
    assert_eq!(regs.read(EReg::<u64>::Zero), 0);
}
