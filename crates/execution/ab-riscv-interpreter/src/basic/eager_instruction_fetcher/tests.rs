//! Tests for what the decoded instruction stream contains and for
//! [`BasicEagerInstructionFetcher`]'s relative branch handling.
//!
//! [`ProgramCounter::set_pc_relative()`] moves within the decoded instruction stream rather than
//! resolving an address and converting it back, and its fast path deliberately skips the checks
//! [`ProgramCounter::set_pc()`] does. Most of the tests below pin down that the cases it skips are
//! still handled: the return trap, branches past the end of the stream, branches off its start and
//! unaligned targets.

use crate::basic::eager_instruction_fetcher::{
    BasicEagerInstructionFetcher, BasicEagerInstructions,
};
use crate::basic::{
    BasicInterpreterState, BasicMemory, BasicRegisters, IllegalEcallSystemInstructionHandler,
};
use crate::rv64::test_utils::TestHart;
use crate::{
    ExecutionError, ExecutionResult, FetchInstructionResult, InstructionFetcher,
    OpaqueThreadedExecutionResult, ProgramCounter, SystemInstructionHandler,
    ThreadedExecutableInstruction,
};
use ab_riscv_macros::instruction;
use ab_riscv_primitives::prelude::*;
use alloc::vec::Vec;
use core::ops::ControlFlow;
use core::{assert_matches, fmt};

const MEMORY_BASE_ADDRESS: u64 = 0x1000;
const MEMORY_SIZE: usize = 4 * 1024;
/// Address of the first instruction of [`code()`]
const BASE_ADDR: u64 = MEMORY_BASE_ADDRESS;

type I = Rv64Instruction<TestHart>;
type Memory = BasicMemory<MEMORY_BASE_ADDRESS, MEMORY_SIZE>;

/// `addi x0, x0, 0`, the canonical `nop`
const NOP: u32 = 0x0000_0013;
/// `jalr x0, 0(x1)`, the canonical `ret`
const RET: u32 = 0x0000_8067;

/// Five 4-byte instructions at `BASE_ADDR`, `BASE_ADDR + 4`, ... `BASE_ADDR + 16`
fn code() -> Vec<u8> {
    [NOP, NOP, NOP, NOP, RET]
        .iter()
        .flat_map(|instruction| instruction.to_le_bytes())
        .collect()
}

/// Address one past the last instruction
const END_ADDR: u64 = BASE_ADDR + 5 * 4;

/// Stored in slots whose bytes do not decode
const FALLBACK: I = Rv64Instruction::Unimp {
    rs1: Reg::ZERO,
    rs2: Reg::ZERO,
};

/// RV64I with compressed instructions, where the second half of a 32-bit instruction gets a slot of
/// its own
#[instruction(inherit = [Rv64Instruction, Rv64ZcaInstruction])]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompressedInstruction<Hart = BasicHart<Reg<u64>>>
where
    Hart: HartConfig, {}

#[instruction]
const impl<Reg, Hart> Instruction for CompressedInstruction<Hart>
where
    Hart: [const] HartConfig<Reg = Reg>,
{
    const OWN_ISA_EXTENSIONS: &'static [IsaExtension] = &[];

    const ALIGNMENT: u8 = align_of::<u32>() as u8;

    type Hart = Hart;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        None
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}

#[instruction]
impl<Reg, Hart> fmt::Display for CompressedInstruction<Hart>
where
    Reg: fmt::Display + Copy,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

/// [`FALLBACK`] for [`CompressedInstruction`]
const COMPRESSED_FALLBACK: CompressedInstruction = CompressedInstruction::Unimp {
    rs1: Reg::ZERO,
    rs2: Reg::ZERO,
};

/// Fetch the next instruction, which is expected to be there.
///
/// # Safety
/// Fetching must be allowed, see [`InstructionFetcher`].
unsafe fn fetch<Instr>(fetcher: &mut BasicEagerInstructionFetcher<'_, Instr>) -> Instr
where
    Instr: Instruction,
{
    let memory = Memory::default();
    // SAFETY: Guaranteed by the caller
    let result =
        unsafe { InstructionFetcher::<Instr, Memory>::fetch_instruction(fetcher, &memory) };
    let FetchInstructionResult::Instruction(instruction) = result else {
        panic!("Expected an instruction");
    };

    instruction
}

/// Decode [`code()`], which [`new_fetcher()`] then walks
fn new_instructions(return_trap_address: u64) -> BasicEagerInstructions<I> {
    // SAFETY: `unimp` never continues to the next instruction, the return trap is outside of the
    // instruction stream and the base address is aligned
    unsafe { BasicEagerInstructions::decode(&code(), FALLBACK, return_trap_address, BASE_ADDR) }
}

#[test]
fn every_alignment_step_of_guest_code_owns_a_slot() {
    // This instruction set has no compressed instructions, so its slots are words rather than
    // halfwords and there is nothing to decode in the middle of an instruction
    assert_eq!(I::ALIGNMENT, size_of::<u32>() as u8);

    let code = code();

    // Including guest code that ends in the middle of an instruction, which decodes as far as
    // whole alignment steps go and leaves the trailing bytes out
    for len in 0..=code.len() {
        // SAFETY: Nothing is executed here, only the decoded stream is inspected, so what
        // execution may reach doesn't come into play
        let instructions =
            unsafe { BasicEagerInstructions::<I>::decode(&code[..len], FALLBACK, 0, BASE_ADDR) };

        assert_eq!(
            instructions.instructions_len(),
            len / usize::from(I::ALIGNMENT),
            "{len} bytes of guest code"
        );
    }
}

#[test]
fn every_slot_holds_the_instruction_its_bytes_decode_to() {
    let instructions = new_instructions(END_ADDR);
    let memory = Memory::default();
    let mut fetcher = instructions
        .fetcher(BASE_ADDR)
        .expect("This is the address of the first instruction of `code()`; qed");

    for encoded_instruction in [NOP, NOP, NOP, NOP, RET] {
        let expected = I::try_decode(encoded_instruction).expect("Valid instruction; qed");

        // SAFETY: The fetcher is new, and every instruction before the next one is a `nop`, which
        // continues
        let result =
            unsafe { InstructionFetcher::<I, Memory>::fetch_instruction(&mut fetcher, &memory) };
        let FetchInstructionResult::Instruction(instruction) = result else {
            panic!("Expected an instruction");
        };

        assert_eq!(instruction, expected);
    }
}

/// Build a fetcher whose program counter sits just after the instruction at `BASE_ADDR + 4`,
/// which is the state relative branches are resolved from: the program counter is advanced
/// during instruction fetching, so `set_pc_relative(_, 4, offset)` branches from
/// `BASE_ADDR + 4`
fn new_fetcher(instructions: &BasicEagerInstructions<I>) -> BasicEagerInstructionFetcher<'_, I> {
    instructions
        .fetcher(BASE_ADDR + 8)
        .expect("Program counter is valid and aligned; qed")
}

#[test]
fn fetcher_only_starts_at_a_decoded_instruction() {
    let instructions = new_instructions(END_ADDR);

    for pc in [BASE_ADDR, BASE_ADDR + 8, END_ADDR - 4] {
        assert!(instructions.fetcher(pc).is_some(), "{pc:#x}");
    }
    // Before the start, past the end, unaligned and far away enough to not fit into `usize` on a
    // 32-bit host
    for pc in [
        BASE_ADDR - 4,
        END_ADDR,
        BASE_ADDR + 2,
        BASE_ADDR + (1 << 40),
        0,
        !3,
    ] {
        assert!(instructions.fetcher(pc).is_none(), "{pc:#x}");
    }
}

/// Branch by `offset` from the instruction at `BASE_ADDR + 4`
fn branch(
    fetcher: &mut BasicEagerInstructionFetcher<'_, I>,
    offset: i32,
) -> Result<ControlFlow<()>, ExecutionError<u64>> {
    let memory = Memory::default();
    fetcher.set_pc_relative(&memory, 4, offset)
}

#[test]
fn forward_and_backward_branches_move_within_the_stream() {
    let instructions = new_instructions(0);
    let mut fetcher = new_fetcher(&instructions);

    assert_matches!(
        branch(&mut fetcher, 8),
        Ok(ControlFlow::Continue(())),
        "Expected Continue"
    );
    assert_eq!(
        ProgramCounter::<u64, Memory>::get_pc(&fetcher),
        BASE_ADDR + 12
    );

    // Now branching from `BASE_ADDR + 8`, back to the very first instruction
    assert_matches!(
        branch(&mut fetcher, -8),
        Ok(ControlFlow::Continue(())),
        "Expected Continue"
    );
    assert_eq!(ProgramCounter::<u64, Memory>::get_pc(&fetcher), BASE_ADDR);
}

#[test]
fn branch_to_the_last_instruction_stays_in_bounds() {
    let instructions = new_instructions(0);
    let mut fetcher = new_fetcher(&instructions);

    assert_matches!(
        branch(&mut fetcher, 12),
        Ok(ControlFlow::Continue(())),
        "Expected Continue"
    );
    assert_eq!(
        ProgramCounter::<u64, Memory>::get_pc(&fetcher),
        END_ADDR - 4
    );
}

#[test]
fn branch_to_a_trap_below_the_stream_stops_execution() {
    // The usual arrangement: the trap sentinel sits outside the guest's code
    let instructions = new_instructions(0);
    let mut fetcher = new_fetcher(&instructions);

    // From `BASE_ADDR + 4` down to address 0
    let offset = -i32::try_from(BASE_ADDR + 4).unwrap();
    assert_matches!(
        branch(&mut fetcher, offset),
        Ok(ControlFlow::Break(())),
        "Expected Break"
    );
}

#[test]
fn jump_to_a_trap_below_the_stream_stops_at_the_trap() {
    // Threaded dispatch can't run on an x86-64 CPU without AVX
    if !OpaqueThreadedExecutionResult::<I>::platform_supported() {
        return;
    }

    // `jal zero, -0x1000`, from the first instruction down to address 0
    assert_eq!(BASE_ADDR, 0x1000);
    let code = 0x800f_f06f_u32.to_le_bytes();
    // SAFETY: `unimp` never continues to the next instruction, the return trap is outside of the
    // instruction stream and the base address is aligned
    let instructions =
        unsafe { BasicEagerInstructions::<I>::decode(&code, FALLBACK, 0, BASE_ADDR) };
    let fetcher = instructions
        .fetcher(BASE_ADDR)
        .expect("This is the address of the first instruction; qed");

    // Execution stops with the program counter of the fetcher after `failed_branch()`
    let result = I::execute_threaded(
        fetcher,
        &mut BasicRegisters::<Reg<u64>>::default(),
        IllegalEcallSystemInstructionHandler,
        &mut Memory::default(),
    );
    assert_matches!(
        result.outcome,
        Ok(()),
        "Unexpected outcome {:?}",
        result.outcome
    );
    assert_eq!(result.program_counter, 0);
}

#[test]
fn branch_to_a_trap_above_the_stream_stops_execution() {
    let instructions = new_instructions(END_ADDR + 4);
    let mut fetcher = new_fetcher(&instructions);

    let offset = i32::try_from(END_ADDR + 4 - (BASE_ADDR + 4)).unwrap();
    assert_matches!(
        branch(&mut fetcher, offset),
        Ok(ControlFlow::Break(())),
        "Expected Break"
    );
}

#[test]
fn branch_past_the_end_of_the_stream_is_out_of_bounds() {
    let instructions = new_instructions(0);
    let mut fetcher = new_fetcher(&instructions);

    let error = branch(&mut fetcher, 1024).unwrap_err();
    assert_matches!(
        error,
        ExecutionError::OutOfBoundsRead { address } if address.get() == BASE_ADDR + 4 + 1024,
        "Unexpected error {error:?}"
    );
}

#[test]
fn branch_off_the_start_of_the_stream_is_out_of_bounds() {
    let instructions = new_instructions(0);
    let mut fetcher = new_fetcher(&instructions);

    // Lands below `BASE_ADDR`, which underflows the stream rather than wrapping into it
    let error = branch(&mut fetcher, -1024).unwrap_err();
    assert_matches!(
        error,
        ExecutionError::OutOfBoundsRead { address } if address.get() == BASE_ADDR + 4 - 1024,
        "Unexpected error {error:?}"
    );
}

#[test]
fn branch_to_an_unaligned_target_is_rejected() {
    // Branch immediates encode a halfword count, so the decoder cannot produce an odd offset
    // and this is not reachable through instruction execution. It is still the rule `set_pc()`
    // applies, and the fast path must not quietly round such a target to a slot boundary
    // instead.
    let instructions = new_instructions(0);
    let mut fetcher = new_fetcher(&instructions);

    let error = branch(&mut fetcher, 3).unwrap_err();
    assert_matches!(
        error,
        ExecutionError::UnalignedInstruction { address } if address.get() == BASE_ADDR + 7,
        "Unexpected error {error:?}"
    );
}

#[test]
fn branch_into_the_middle_of_an_instruction_is_rejected() {
    // This instruction set has no compressed instructions, so a halfword-aligned target lands
    // between two instructions and must be refused rather than rounded to one
    assert_eq!(I::ALIGNMENT, size_of::<u32>() as u8);

    let instructions = new_instructions(0);
    let mut fetcher = new_fetcher(&instructions);

    let error = branch(&mut fetcher, 2).unwrap_err();
    assert_matches!(
        error,
        ExecutionError::UnalignedInstruction { address } if address.get() == BASE_ADDR + 6,
        "Unexpected error {error:?}"
    );
}

#[test]
fn jump_far_past_the_end_of_the_stream_is_out_of_bounds() {
    let instructions = new_instructions(0);
    let mut fetcher = new_fetcher(&instructions);
    let memory = Memory::default();

    // Exactly 4 GiB past the first instruction, which is the first instruction again for a 32-bit
    // offset
    let address = BASE_ADDR + (1 << 32);
    let error = fetcher.set_pc(&memory, address).unwrap_err();
    assert_matches!(
        error,
        ExecutionError::OutOfBoundsRead { address: error_address } if error_address.get() == address,
        "Unexpected error {error:?}"
    );
}

#[test]
fn branch_that_wraps_around_the_address_space_is_out_of_bounds() {
    let instructions = new_instructions(0);
    let mut fetcher = new_fetcher(&instructions);

    // Far enough back that the guest address itself wraps around zero
    let error = branch(&mut fetcher, i32::MIN).unwrap_err();
    assert_matches!(
        error,
        ExecutionError::OutOfBoundsRead { address: _ },
        "Unexpected error {error:?}"
    );
}

#[test]
fn falling_through_the_end_reaches_the_fallback() {
    // Doesn't end with a jump
    let code = NOP.to_le_bytes();
    // SAFETY: `unimp` never continues to the next instruction, the return trap is outside of the
    // instruction stream and the base address is aligned
    let instructions =
        unsafe { BasicEagerInstructions::<I>::decode(&code, FALLBACK, 0, BASE_ADDR) };
    let mut fetcher = instructions
        .fetcher(BASE_ADDR)
        .expect("This is the address of the first instruction; qed");

    assert_eq!(
        // SAFETY: The fetcher is new
        unsafe { fetch(&mut fetcher) },
        I::try_decode(NOP).expect("Valid instruction; qed")
    );
    // SAFETY: The `nop` fetched above continues
    assert_eq!(unsafe { fetch(&mut fetcher) }, FALLBACK);
}

#[test]
fn jump_into_the_second_half_of_the_last_instruction_falls_through_to_the_fallback() {
    // `jal zero, 2` jumps into its own second half, which decodes as `c.addi4spn s0, sp, 8`
    let code = 0x0020_006f_u32.to_le_bytes();
    // SAFETY: `unimp` never continues to the next instruction, the return trap is outside of the
    // instruction stream and the base address is aligned
    let instructions = unsafe {
        BasicEagerInstructions::<CompressedInstruction>::decode(
            &code,
            COMPRESSED_FALLBACK,
            0,
            BASE_ADDR,
        )
    };
    let mut fetcher = instructions
        .fetcher(BASE_ADDR + 2)
        .expect("Second half of an instruction is aligned; qed");

    assert_eq!(
        // SAFETY: The fetcher is new
        unsafe { fetch(&mut fetcher) },
        <CompressedInstruction>::try_decode(0x0020).expect("Valid instruction; qed")
    );
    // SAFETY: The `c.addi4spn` fetched above continues
    assert_eq!(unsafe { fetch(&mut fetcher) }, COMPRESSED_FALLBACK);
}

#[test]
fn instruction_longer_than_guest_code_left_is_not_decoded() {
    // `c.nop` followed by the first half of `jal zero, 0`, which only decodes when padded with
    // zeroes that are not in the guest code
    let code = [0x0001_u16.to_le_bytes(), 0x006f_u16.to_le_bytes()].concat();
    assert!(<CompressedInstruction>::try_decode(0x006f).is_some());
    // SAFETY: `unimp` never continues to the next instruction, the return trap is outside of the
    // instruction stream and the base address is aligned
    let instructions = unsafe {
        BasicEagerInstructions::<CompressedInstruction>::decode(
            &code,
            COMPRESSED_FALLBACK,
            0,
            BASE_ADDR,
        )
    };
    let mut fetcher = instructions
        .fetcher(BASE_ADDR + 2)
        .expect("This is the address of the second instruction; qed");

    // SAFETY: The fetcher is new
    assert_eq!(unsafe { fetch(&mut fetcher) }, COMPRESSED_FALLBACK);
}

/// Start execution at `fetcher` with the loop of [`BasicInterpreterState`] and, where the platform
/// supports it, with threaded dispatch, and return what each one stopped with and its program
/// counter
fn execute_from<'a, Env>(
    fetcher: BasicEagerInstructionFetcher<'a, I>,
    regs: BasicRegisters<Reg<u64>>,
    env: Env,
) -> Vec<(Result<(), ExecutionError<u64>>, u64)>
where
    Env: Copy
        + SystemInstructionHandler<
            Reg<u64>,
            BasicRegisters<Reg<u64>>,
            Memory,
            BasicEagerInstructionFetcher<'a, I>,
        >,
    I: ThreadedExecutableInstruction<
            BasicRegisters<Reg<u64>>,
            Env,
            Memory,
            BasicEagerInstructionFetcher<'a, I>,
        >,
{
    let mut results = Vec::with_capacity(2);

    let mut state = BasicInterpreterState {
        regs,
        env,
        memory: Memory::default(),
        instruction_fetcher: fetcher,
    };
    let outcome = state.execute::<I>();
    results.push((
        outcome,
        ProgramCounter::<u64, Memory>::get_pc(&state.instruction_fetcher),
    ));

    // Threaded dispatch can't run on an x86-64 CPU without AVX
    if OpaqueThreadedExecutionResult::<I>::platform_supported() {
        let result = I::execute_threaded(fetcher, &mut regs.clone(), env, &mut Memory::default());
        results.push((result.outcome, result.program_counter));
    }

    results
}

/// Start execution again at `fetcher` with default registers and an environment that refuses
/// `ecall`, see [`execute_from()`]
fn execute_again(
    fetcher: BasicEagerInstructionFetcher<'_, I>,
) -> Vec<(Result<(), ExecutionError<u64>>, u64)> {
    execute_from(
        fetcher,
        BasicRegisters::default(),
        IllegalEcallSystemInstructionHandler,
    )
}

#[test]
fn execution_after_a_branch_past_the_end_of_the_stream_fails_right_away() {
    let instructions = new_instructions(0);
    let mut fetcher = new_fetcher(&instructions);
    let target = BASE_ADDR + 4 + 1024;

    assert_matches!(branch(&mut fetcher, 1024), Err(_), "Expected an error");
    // Nothing is fetched from where the refused branch left the program counter, and it stays there
    for (outcome, pc) in execute_again(fetcher) {
        assert_matches!(
            outcome,
            Err(ExecutionError::OutOfBoundsRead { address }) if address.get() == target,
            "Unexpected outcome {outcome:?}"
        );
        assert_eq!(pc, target);
    }
}

#[test]
fn execution_after_a_branch_off_the_start_of_the_stream_fails_right_away() {
    let instructions = new_instructions(0);
    let mut fetcher = new_fetcher(&instructions);
    let target = BASE_ADDR + 4 - 1024;

    assert_matches!(branch(&mut fetcher, -1024), Err(_), "Expected an error");
    for (outcome, pc) in execute_again(fetcher) {
        assert_matches!(
            outcome,
            Err(ExecutionError::OutOfBoundsRead { address }) if address.get() == target,
            "Unexpected outcome {outcome:?}"
        );
        assert_eq!(pc, target);
    }
}

#[test]
fn execution_after_a_branch_into_the_middle_of_an_instruction_fails_right_away() {
    let instructions = new_instructions(0);
    let mut fetcher = new_fetcher(&instructions);
    let target = BASE_ADDR + 6;

    // The position is in the middle of a decoded instruction, which is not one to fetch either
    assert_matches!(branch(&mut fetcher, 2), Err(_), "Expected an error");
    for (outcome, pc) in execute_again(fetcher) {
        assert_matches!(
            outcome,
            Err(ExecutionError::UnalignedInstruction { address }) if address.get() == target,
            "Unexpected outcome {outcome:?}"
        );
        assert_eq!(pc, target);
    }
}

#[test]
fn execution_after_a_branch_to_the_trap_stops_right_away() {
    let instructions = new_instructions(0);
    let mut fetcher = new_fetcher(&instructions);

    // From `BASE_ADDR + 4` down to address 0
    let offset = -i32::try_from(BASE_ADDR + 4).unwrap();
    assert_matches!(
        branch(&mut fetcher, offset),
        Ok(ControlFlow::Break(())),
        "Expected Break"
    );
    for (outcome, pc) in execute_again(fetcher) {
        assert_matches!(outcome, Ok(()), "Unexpected outcome {outcome:?}");
        assert_eq!(pc, 0);
    }
}

#[test]
fn execution_that_starts_again_past_the_fallback_fails_right_away() {
    // Doesn't end with a jump, so execution falls through to the fallback instruction
    let code = NOP.to_le_bytes();
    // SAFETY: `unimp` never continues to the next instruction, the return trap is outside of the
    // instruction stream and the base address is aligned
    let instructions =
        unsafe { BasicEagerInstructions::<I>::decode(&code, FALLBACK, 0, BASE_ADDR) };
    let fetcher = instructions
        .fetcher(BASE_ADDR)
        .expect("This is the address of the first instruction; qed");

    let mut state = BasicInterpreterState {
        regs: BasicRegisters::<Reg<u64>>::default(),
        env: IllegalEcallSystemInstructionHandler,
        memory: Memory::default(),
        instruction_fetcher: fetcher,
    };
    let outcome = state.execute::<I>();
    assert_matches!(
        outcome,
        Err(ExecutionError::IllegalInstruction { address }) if address.get() == BASE_ADDR + 4,
        "Unexpected outcome {outcome:?}"
    );

    // Fetching the fallback instruction moved the program counter past it, out of the decoded
    // stream
    for (outcome, pc) in execute_again(state.instruction_fetcher) {
        assert_matches!(
            outcome,
            Err(ExecutionError::OutOfBoundsRead { address }) if address.get() == BASE_ADDR + 8,
            "Unexpected outcome {outcome:?}"
        );
        assert_eq!(pc, BASE_ADDR + 8);
    }
}

/// `ecall`
const ECALL: u32 = 0x0000_0073;

/// Environment whose `ecall` handler continues at an address, the way a trap handler is entered
#[derive(Debug, Clone, Copy)]
struct JumpingEcallHandler {
    /// Address to continue at
    target: u64,
}

impl<Regs, Memory, PC> SystemInstructionHandler<Reg<u64>, Regs, Memory, PC> for JumpingEcallHandler
where
    PC: ProgramCounter<u64, Memory>,
{
    fn handle_ecall(
        &mut self,
        _regs: &mut Regs,
        _memory: &mut Memory,
        _program_counter: &PC,
    ) -> ExecutionResult<Reg<u64>> {
        ExecutionResult::Jump {
            target: self.target,
        }
    }
}

/// Run `ecall` at `BASE_ADDR` followed by an illegal instruction and `ret` with `ra` holding zero,
/// which is the return trap, see [`execute_from()`]
fn ecall_with(handler: JumpingEcallHandler) -> Vec<(Result<(), ExecutionError<u64>>, u64)> {
    let code = [ECALL, 0, RET]
        .iter()
        .flat_map(|instruction| instruction.to_le_bytes())
        .collect::<Vec<_>>();
    // SAFETY: `unimp` never continues to the next instruction, the return trap is outside of the
    // instruction stream and the base address is aligned
    let instructions =
        unsafe { BasicEagerInstructions::<I>::decode(&code, FALLBACK, 0, BASE_ADDR) };
    let fetcher = instructions
        .fetcher(BASE_ADDR)
        .expect("This is the address of the first instruction; qed");

    execute_from(fetcher, BasicRegisters::default(), handler)
}

#[test]
fn execution_continues_where_the_ecall_handler_jumps() {
    // Over the illegal instruction to `ret`
    let handler = JumpingEcallHandler {
        target: BASE_ADDR + 8,
    };
    for (outcome, _pc) in ecall_with(handler) {
        assert_matches!(outcome, Ok(()), "Unexpected outcome {outcome:?}");
    }
}

#[test]
fn execution_fails_when_the_ecall_handler_jumps_out_of_the_stream() {
    let target = BASE_ADDR + 1024;

    for (outcome, _pc) in ecall_with(JumpingEcallHandler { target }) {
        assert_matches!(
            outcome,
            Err(ExecutionError::OutOfBoundsRead { address }) if address.get() == target,
            "Unexpected outcome {outcome:?}"
        );
    }
}
