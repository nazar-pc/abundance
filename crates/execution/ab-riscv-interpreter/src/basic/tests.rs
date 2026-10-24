extern crate alloc;

use crate::basic::{
    BasicInstructionFetcher, BasicInterpreterState, BasicMemory, BasicRegister, BasicRegisters,
    IllegalEcallSystemInstructionHandler, RegisterOffset,
};
use crate::rv64::test_utils::TestHart;
use crate::{
    ExecutionError, OpaqueThreadedExecutionResult, ProgramCounter, RegisterFile,
    ThreadedExecutableInstruction,
};
use ab_riscv_primitives::prelude::*;
use alloc::vec::Vec;
use core::{assert_matches, fmt};

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

const MEMORY_BASE_ADDRESS: u64 = 0x1000;
type Memory = BasicMemory<MEMORY_BASE_ADDRESS, 4096>;
type I = Rv64Instruction<TestHart>;

/// `jalr x0, 0(x1)`, the canonical `ret`
const RET: u32 = 0x0000_8067;

/// Start execution at `fetcher` with two `ret` instructions at `MEMORY_BASE_ADDRESS`, with the loop
/// of [`BasicInterpreterState`] and, where the platform supports it, with threaded dispatch, and
/// return what each one stopped with and its program counter
fn execute_from(
    fetcher: BasicInstructionFetcher<I>,
) -> Vec<(Result<(), ExecutionError<u64>>, u64)> {
    let mut memory = Memory::default();
    memory
        .get_mut_bytes(MEMORY_BASE_ADDRESS, 2 * size_of::<u32>())
        .expect("Within memory; qed")
        .copy_from_slice(&[RET.to_le_bytes(), RET.to_le_bytes()].concat());
    let mut results = Vec::with_capacity(2);

    let mut state = BasicInterpreterState {
        regs: BasicRegisters::<Reg<u64>>::default(),
        env: IllegalEcallSystemInstructionHandler,
        memory,
        instruction_fetcher: fetcher,
    };
    let outcome = state.execute::<I>();
    results.push((
        outcome,
        ProgramCounter::<u64, Memory>::get_pc(&state.instruction_fetcher),
    ));

    // Threaded dispatch can't run on an x86-64 CPU without AVX
    if OpaqueThreadedExecutionResult::<I>::platform_supported() {
        let result = I::execute_threaded(
            fetcher,
            &mut BasicRegisters::<Reg<u64>>::default(),
            IllegalEcallSystemInstructionHandler,
            &mut memory,
        );
        results.push((result.outcome, result.program_counter));
    }

    results
}

#[test]
fn execution_that_starts_unaligned_fails_right_away() {
    let pc = MEMORY_BASE_ADDRESS + 2;

    // Nothing is fetched from the middle of the first `ret`
    for (outcome, stopped_at) in execute_from(BasicInstructionFetcher::new(0, pc)) {
        assert_matches!(
            outcome,
            Err(ExecutionError::UnalignedInstruction { address }) if address.get() == pc,
            "Unexpected outcome {outcome:?}"
        );
        assert_eq!(stopped_at, pc);
    }
}

#[test]
fn execution_that_starts_at_the_trap_stops_right_away() {
    // Nothing is fetched from the trap, which is outside of memory
    for (outcome, stopped_at) in execute_from(BasicInstructionFetcher::new(0, 0)) {
        assert_matches!(outcome, Ok(()), "Unexpected outcome {outcome:?}");
        assert_eq!(stopped_at, 0);
    }
}

/// Run `ret` at `MEMORY_BASE_ADDRESS` with `ra` holding `return_trap_address` through
/// [`BasicInstructionFetcher`], with threaded dispatch and with the loop of
/// [`BasicInterpreterState`], and return the program counter each one stops with. The last one is
/// from the loop started again where it stopped.
fn ret_to_trap(return_trap_address: u64) -> [u64; 3] {
    let mut memory = Memory::default();
    memory
        .get_mut_bytes(MEMORY_BASE_ADDRESS, size_of::<u32>())
        .expect("Within memory; qed")
        .copy_from_slice(&RET.to_le_bytes());
    let fetcher = BasicInstructionFetcher::<I>::new(return_trap_address, MEMORY_BASE_ADDRESS);
    let mut regs = BasicRegisters::<Reg<u64>>::default();
    regs.write(Reg::Ra, return_trap_address);

    let result = I::execute_threaded(
        fetcher,
        &mut regs.clone(),
        IllegalEcallSystemInstructionHandler,
        &mut memory.clone(),
    );
    assert_matches!(
        result.outcome,
        Ok(()),
        "Unexpected outcome {:?}",
        result.outcome
    );

    let mut state = BasicInterpreterState {
        regs,
        env: IllegalEcallSystemInstructionHandler,
        memory,
        instruction_fetcher: fetcher,
    };
    let outcome = state.execute::<I>();
    assert_matches!(outcome, Ok(()), "Unexpected outcome {outcome:?}");
    let stopped_at = ProgramCounter::<u64, Memory>::get_pc(&state.instruction_fetcher);

    // Starting again at the trap stops right away
    let outcome = state.execute::<I>();
    assert_matches!(outcome, Ok(()), "Unexpected outcome {outcome:?}");

    [
        result.program_counter,
        stopped_at,
        ProgramCounter::<u64, Memory>::get_pc(&state.instruction_fetcher),
    ]
}

#[test]
fn absolute_jump_to_a_trap_below_the_code_stops_at_the_trap() {
    // Threaded dispatch can't run on an x86-64 CPU without AVX
    if !OpaqueThreadedExecutionResult::<I>::platform_supported() {
        return;
    }

    assert_eq!(ret_to_trap(0), [0; 3]);
}

#[test]
fn absolute_jump_to_a_trap_above_the_code_stops_at_the_trap() {
    // Threaded dispatch can't run on an x86-64 CPU without AVX
    if !OpaqueThreadedExecutionResult::<I>::platform_supported() {
        return;
    }

    let return_trap_address = MEMORY_BASE_ADDRESS + 0x100;
    assert_eq!(ret_to_trap(return_trap_address), [return_trap_address; 3]);
}
