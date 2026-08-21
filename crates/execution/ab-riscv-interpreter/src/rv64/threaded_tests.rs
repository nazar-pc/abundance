//! Tail-call-threaded dispatch is generated from the same instruction implementations the
//! `match`-based [`ExecutableInstruction::execute()`] is generated from, so it is worth testing
//! both paths agree, all the way through register writes, branches, jumps and failures.

extern crate alloc;

use crate::basic::BasicRegisters;
use crate::rv64::test_utils::{
    ExtState, TEST_BASE_ADDR, TestInstructionFetcher, TestInstructionHandler, TestMemory, execute,
    execute_threaded, initialize_state,
};
use crate::{
    ExecutableInstruction, ExecutionError, OpaqueThreadedExecutionResult, ProgramCounter,
    RegisterFile, ThreadedExecutableInstruction, VirtualMemory,
};
use ab_riscv_primitives::prelude::*;
use alloc::vec;
use alloc::vec::Vec;

/// Whether threaded dispatch can run here at all.
///
/// It cannot on an x86-64 CPU without AVX, see
/// [`OpaqueThreadedExecutionResult::platform_supported()`]. Miri emulates exactly such a CPU, but
/// the lanes fall back to a plain array there, so this is only ever `false` on real pre-2011
/// hardware; the tests below check the reported reason in that case rather than skipping silently.
fn threaded_dispatch_available<I>() -> bool
where
    I: Instruction,
{
    if OpaqueThreadedExecutionResult::<I>::platform_supported() {
        return true;
    }

    let mut state = initialize_state(vec![]);

    assert!(
        matches!(
            execute_threaded::<Rv64Instruction<Reg<u64>>>(&mut state).outcome,
            Err(ExecutionError::UnsupportedPlatform)
        ),
        "Threaded dispatch must say why it did not run"
    );

    false
}

/// Runs `instructions` twice, once through each dispatch path, and asserts that the two agree on
/// the error (if any), on every general purpose register and on the final program counter
fn assert_paths_agree<I, Instructions>(
    instructions: Instructions,
    setup: fn(&mut BasicRegisters<Reg<u64>>),
) where
    I: Instruction<Reg = Reg<u64>>
        + ExecutableInstruction<
            BasicRegisters<Reg<u64>>,
            ExtState,
            TestMemory,
            TestInstructionFetcher<I>,
            TestInstructionHandler,
        > + for<'a> ThreadedExecutableInstruction<
            BasicRegisters<Reg<u64>>,
            &'a mut ExtState,
            TestMemory,
            TestInstructionFetcher<I>,
            &'a mut TestInstructionHandler,
        >,
    Instructions: IntoIterator<Item = I> + Clone,
{
    if !threaded_dispatch_available::<I>() {
        return;
    }

    let mut looped_state = initialize_state(instructions.clone());
    setup(&mut looped_state.regs);
    let looped_result = execute(&mut looped_state);

    let mut threaded_state = initialize_state(instructions);
    setup(&mut threaded_state.regs);
    let threaded_result = execute_threaded(&mut threaded_state);

    assert_eq!(
        alloc::format!("{looped_result:?}"),
        alloc::format!("{:?}", threaded_result.outcome),
        "Dispatch paths disagree on the outcome"
    );

    for bits in 0..32 {
        let reg = Reg::<u64>::from_bits(bits).unwrap();
        assert_eq!(
            looped_state.regs.read(reg),
            threaded_state.regs.read(reg),
            "Dispatch paths disagree on register {bits}"
        );
    }

    // The threaded path is handed a clone of the fetcher and hands back only where it stopped, so
    // the state's own fetcher stays where it was and this compares against what came back
    assert_eq!(
        ProgramCounter::<u64, TestMemory>::get_pc(&looped_state.instruction_fetcher),
        threaded_result.program_counter,
        "Dispatch paths disagree on the program counter"
    );
}

#[test]
fn threaded_matches_looped_arithmetic() {
    assert_paths_agree(
        vec![
            Rv64Instruction::Add {
                rd: Reg::A2,
                rs1: Reg::A0,
                rs2: Reg::A1,
            },
            Rv64Instruction::Sub {
                rd: Reg::A3,
                rs1: Reg::A0,
                rs2: Reg::A1,
            },
            // Writes to the zero register have to be discarded on both paths
            Rv64Instruction::Add {
                rd: Reg::Zero,
                rs1: Reg::A0,
                rs2: Reg::A1,
            },
            // Neither source register is used, so the handler for this one reads neither
            Rv64Instruction::Lui {
                rd: Reg::A4,
                imm: I24WithZeroedBits::from_i32(0x1000),
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
        ],
        |regs| {
            regs.write(Reg::A0, 10);
            regs.write(Reg::A1, 20);
        },
    );
}

#[test]
fn threaded_matches_looped_memory() {
    assert_paths_agree(
        vec![
            Rv64Instruction::Sd {
                rs1: Reg::A0,
                rs2: Reg::A1,
                imm: 0,
            },
            Rv64Instruction::Ld {
                rd: Reg::A2,
                rs1: Reg::A0,
                imm: 0,
                rs2: Reg::Zero,
            },
        ],
        |regs| {
            regs.write(Reg::A0, TEST_BASE_ADDR);
            regs.write(Reg::A1, 0x0123_4567_89ab_cdef);
        },
    );
}

#[test]
fn threaded_matches_looped_taken_branch() {
    assert_paths_agree(
        vec![
            // Skips over the `Addi` below when the branch is taken
            Rv64Instruction::Beq {
                rs1: Reg::A0,
                rs2: Reg::A1,
                imm: I24::from_i32(8),
            },
            Rv64Instruction::Addi {
                rd: Reg::A2,
                rs1: Reg::Zero,
                imm: 1,
                rs2: Reg::Zero,
            },
            Rv64Instruction::Addi {
                rd: Reg::A3,
                rs1: Reg::Zero,
                imm: 2,
                rs2: Reg::Zero,
            },
        ],
        |regs| {
            regs.write(Reg::A0, 7);
            regs.write(Reg::A1, 7);
        },
    );
}

#[test]
fn threaded_matches_looped_untaken_branch() {
    assert_paths_agree(
        vec![
            Rv64Instruction::Beq {
                rs1: Reg::A0,
                rs2: Reg::A1,
                imm: I24::from_i32(8),
            },
            Rv64Instruction::Addi {
                rd: Reg::A2,
                rs1: Reg::Zero,
                imm: 1,
                rs2: Reg::Zero,
            },
            Rv64Instruction::Addi {
                rd: Reg::A3,
                rs1: Reg::Zero,
                imm: 2,
                rs2: Reg::Zero,
            },
        ],
        |regs| {
            regs.write(Reg::A0, 7);
            regs.write(Reg::A1, 8);
        },
    );
}

#[test]
fn threaded_matches_looped_jump() {
    assert_paths_agree(
        vec![
            Rv64Instruction::Jal {
                rd: Reg::Ra,
                imm: I24::from_i32(8),
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            Rv64Instruction::Addi {
                rd: Reg::A2,
                rs1: Reg::Zero,
                imm: 1,
                rs2: Reg::Zero,
            },
            Rv64Instruction::Addi {
                rd: Reg::A3,
                rs1: Reg::Zero,
                imm: 2,
                rs2: Reg::Zero,
            },
        ],
        |_regs| {},
    );
}

#[test]
fn threaded_matches_looped_failure() {
    // Both paths have to stop at the same instruction with the same error and leave the registers
    // written by the instructions before it
    assert_paths_agree(
        vec![
            Rv64Instruction::Addi {
                rd: Reg::A2,
                rs1: Reg::Zero,
                imm: 42,
                rs2: Reg::Zero,
            },
            Rv64Instruction::Ld {
                rd: Reg::A3,
                rs1: Reg::A0,
                imm: 0,
                rs2: Reg::Zero,
            },
            Rv64Instruction::Addi {
                rd: Reg::A4,
                rs1: Reg::Zero,
                imm: 1,
                rs2: Reg::Zero,
            },
        ],
        |regs| {
            regs.write(Reg::A0, 0xdead_0000);
        },
    );
}

#[test]
fn threaded_reports_the_error_it_stopped_on() {
    if !threaded_dispatch_available::<Rv64Instruction<Reg<u64>>>() {
        return;
    }

    let mut state = initialize_state(vec![Rv64Instruction::Ld {
        rd: Reg::A3,
        rs1: Reg::A0,
        imm: 0,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::A0, 0xdead_0000);

    assert!(matches!(
        execute_threaded(&mut state).outcome,
        Err(ExecutionError::OutOfBoundsRead { .. })
    ));
}

#[test]
fn threaded_runs_a_loop_to_completion() {
    if !threaded_dispatch_available::<Rv64Instruction<Reg<u64>>>() {
        return;
    }

    // Counts down from 1000, which is more iterations than any stack could hold if the handler
    // chain were not made of real tail calls
    let instructions = vec![
        Rv64Instruction::Addi {
            rd: Reg::A0,
            rs1: Reg::A0,
            imm: -1,
            rs2: Reg::Zero,
        },
        Rv64Instruction::Bne {
            rs1: Reg::A0,
            rs2: Reg::Zero,
            imm: I24::from_i32(-4),
        },
    ];

    let mut state = initialize_state(instructions);
    state.regs.write(Reg::A0, 1000);

    execute_threaded(&mut state).outcome.unwrap();

    assert_eq!(state.regs.read(Reg::A0), 0);
}

#[test]
fn threaded_leaves_memory_in_the_same_state() {
    if !threaded_dispatch_available::<Rv64Instruction<Reg<u64>>>() {
        return;
    }

    let instructions: Vec<_> = (0..8)
        .map(|index| Rv64Instruction::Sd {
            rs1: Reg::A0,
            rs2: Reg::A1,
            imm: index * 8,
        })
        .collect();

    let mut looped_state = initialize_state(instructions.clone());
    looped_state.regs.write(Reg::A0, TEST_BASE_ADDR);
    looped_state.regs.write(Reg::A1, 0xabcd);
    execute(&mut looped_state).unwrap();

    let mut threaded_state = initialize_state(instructions);
    threaded_state.regs.write(Reg::A0, TEST_BASE_ADDR);
    threaded_state.regs.write(Reg::A1, 0xabcd);
    execute_threaded(&mut threaded_state).outcome.unwrap();

    for index in 0..8 {
        let address = TEST_BASE_ADDR + index * 8;
        assert_eq!(
            looped_state.memory.read::<u64>(address).unwrap(),
            threaded_state.memory.read::<u64>(address).unwrap(),
            "Dispatch paths disagree on memory at {address:#x}"
        );
    }
}
