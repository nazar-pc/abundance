use ab_riscv_interpreter::rv64::{Rv64InterpreterState, Rv64SystemInstructionHandler};
use ab_riscv_interpreter::{
    BasicInt, ExecutableInstruction, ExecutionError, FetchInstructionResult, InstructionFetcher,
    ProgramCounter, ProgramCounterError, VirtualMemory, VirtualMemoryError,
};
use ab_riscv_primitives::instructions::Instruction;
use ab_riscv_primitives::instructions::rv64::Rv64Instruction;
use ab_riscv_primitives::registers::{Reg, Register, Registers};
use core::marker::PhantomData;
use core::ops::ControlFlow;
use std::path::Path;
use std::{fs, str};

pub(super) const TEST_BASE_ADDR: u64 = 0;
const TRAP_ADDRESS: u64 = u64::MAX << size_of::<u32>().ilog2();

type Address<I> = <<I as Instruction>::Reg as Register>::Type;

/// Simple test memory implementation
pub(super) struct TestMemory {
    data: Vec<u8>,
    base_addr: u64,
}

impl TestMemory {
    fn new(size: usize, base_addr: u64) -> Self {
        Self {
            data: vec![0; size],
            base_addr,
        }
    }
}

impl VirtualMemory for TestMemory {
    fn read<T>(&self, address: u64) -> Result<T, VirtualMemoryError>
    where
        T: BasicInt,
    {
        let offset = address
            .checked_sub(self.base_addr)
            .ok_or(VirtualMemoryError::OutOfBoundsRead { address })? as usize;

        if offset + size_of::<T>() > self.data.len() {
            return Err(VirtualMemoryError::OutOfBoundsRead { address });
        }

        // SAFETY: Only reading basic integers from initialized memory
        unsafe {
            Ok(self
                .data
                .as_ptr()
                .cast::<T>()
                .byte_add(offset)
                .read_unaligned())
        }
    }

    unsafe fn read_unchecked<T>(&self, address: u64) -> T
    where
        T: BasicInt,
    {
        // SAFETY: Guaranteed by function contract
        unsafe {
            let offset = address.unchecked_sub(self.base_addr) as usize;
            self.data
                .as_ptr()
                .cast::<T>()
                .byte_add(offset)
                .read_unaligned()
        }
    }

    fn write<T>(&mut self, address: u64, value: T) -> Result<(), VirtualMemoryError>
    where
        T: BasicInt,
    {
        let offset = address
            .checked_sub(self.base_addr)
            .ok_or(VirtualMemoryError::OutOfBoundsWrite { address })? as usize;

        if offset + size_of::<T>() > self.data.len() {
            return Err(VirtualMemoryError::OutOfBoundsWrite { address });
        }

        // SAFETY: Only writing basic integers to initialized memory
        unsafe {
            self.data
                .as_mut_ptr()
                .cast::<T>()
                .byte_add(offset)
                .write_unaligned(value);
        }

        Ok(())
    }
}

/// Custom instruction handler for tests that returns instructions from a sequence
pub(super) struct TestInstructionFetcher<I> {
    instructions: Vec<I>,
    return_trap_address: u64,
    base_address: u64,
    pc: u64,
}

impl<I> ProgramCounter<u64, TestMemory, &'static str> for TestInstructionFetcher<I>
where
    I: Instruction<Reg = Reg<u64>>,
{
    #[inline(always)]
    fn get_pc(&self) -> u64 {
        self.pc
    }

    fn set_pc(
        &mut self,
        _memory: &mut TestMemory,
        pc: u64,
    ) -> Result<ControlFlow<()>, ProgramCounterError<u64, &'static str>> {
        self.pc = pc;

        Ok(ControlFlow::Continue(()))
    }
}

impl<I> InstructionFetcher<I, TestMemory, &'static str> for TestInstructionFetcher<I>
where
    I: Instruction<Reg = Reg<u64>>,
{
    #[inline]
    fn fetch_instruction(
        &mut self,
        _memory: &mut TestMemory,
    ) -> Result<FetchInstructionResult<I>, ExecutionError<Address<I>, I, &'static str>> {
        if self.pc == self.return_trap_address {
            return Ok(FetchInstructionResult::ControlFlow(ControlFlow::Break(())));
        }

        let Some(&instruction) = self
            .instructions
            .get((self.pc - self.base_address) as usize / size_of::<u32>())
        else {
            return Ok(FetchInstructionResult::ControlFlow(ControlFlow::Break(())));
        };
        self.pc += 4;

        Ok(FetchInstructionResult::Instruction(instruction))
    }
}

pub(super) struct TestInstructionHandler;

impl<I> Rv64SystemInstructionHandler<Reg<u64>, TestMemory, TestInstructionFetcher<I>, &'static str>
    for TestInstructionHandler
where
    I: Instruction<Reg = Reg<u64>>,
{
    #[inline(always)]
    fn handle_ecall(
        &mut self,
        _regs: &mut Registers<Reg<u64>>,
        _memory: &mut TestMemory,
        program_counter: &mut TestInstructionFetcher<I>,
    ) -> Result<ControlFlow<()>, ExecutionError<u64, Rv64Instruction<Reg<u64>>, &'static str>> {
        let instruction = Rv64Instruction::Ecall;
        Err(ExecutionError::UnsupportedInstruction {
            address: program_counter.get_pc() - u64::from(instruction.size()),
            instruction,
        })
    }
}

impl<I> TestInstructionFetcher<I> {
    /// Create a new instance
    #[inline(always)]
    fn new(instructions: Vec<I>, return_trap_address: u64, base_address: u64, pc: u64) -> Self {
        Self {
            instructions,
            return_trap_address,
            base_address,
            pc,
        }
    }
}

pub(super) type TestInterpreterState<Instruction> = Rv64InterpreterState<
    Reg<u64>,
    TestMemory,
    TestInstructionFetcher<Instruction>,
    TestInstructionHandler,
    &'static str,
>;

pub(super) fn initialize_state<Instruction, Instructions>(
    instructions: Instructions,
) -> TestInterpreterState<Instruction>
where
    Instructions: Into<Vec<Instruction>>,
{
    Rv64InterpreterState {
        regs: Registers::default(),
        memory: TestMemory::new(8192, TEST_BASE_ADDR),
        instruction_fetcher: TestInstructionFetcher::new(
            instructions.into(),
            TRAP_ADDRESS,
            TEST_BASE_ADDR,
            TEST_BASE_ADDR,
        ),
        system_instruction_handler: TestInstructionHandler,
        _phantom: PhantomData,
    }
}

pub(super) fn execute<I>(
    state: &mut TestInterpreterState<I>,
) -> Result<(), ExecutionError<Address<I>, I, &'static str>>
where
    I: Instruction<Reg = Reg<u64>> + ExecutableInstruction<TestInterpreterState<I>, &'static str>,
{
    loop {
        let instruction = match state
            .instruction_fetcher
            .fetch_instruction(&mut state.memory)?
        {
            FetchInstructionResult::Instruction(instruction) => instruction,
            FetchInstructionResult::ControlFlow(ControlFlow::Continue(())) => {
                continue;
            }
            FetchInstructionResult::ControlFlow(ControlFlow::Break(())) => {
                break;
            }
        };

        match instruction.execute(state)? {
            ControlFlow::Continue(()) => {
                continue;
            }
            ControlFlow::Break(()) => {
                break;
            }
        }
    }

    Ok(())
}

#[derive(Debug, Copy, Clone)]
pub(super) enum RunTestArg {
    Reg(Reg<u64>),
    I16(i16),
    I32(i32),
    Dummy,
}

impl RunTestArg {
    pub(super) fn into_reg(self) -> Reg<u64> {
        if let RunTestArg::Reg(reg) = self {
            reg
        } else {
            panic!("{self:?}");
        }
    }

    pub(super) fn into_i16(self) -> i16 {
        if let RunTestArg::I16(n) = self {
            n
        } else {
            panic!("{self:?}");
        }
    }

    pub(super) fn into_i32(self) -> i32 {
        if let RunTestArg::I32(n) = self {
            n
        } else {
            panic!("{self:?}");
        }
    }
}

pub(super) fn run_tests<I>(
    base_path: &Path,
    map_instruction_name: fn(&str, RunTestArg, RunTestArg, RunTestArg) -> Option<I>,
) where
    I: Instruction<Reg = Reg<u64>> + ExecutableInstruction<TestInterpreterState<I>, &'static str>,
{
    let mut case_count = 0;

    for maybe_entry in fs::read_dir(base_path).unwrap() {
        let path = maybe_entry.unwrap().path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("S") {
            continue;
        }
        let file_name = path.file_name().unwrap().to_str().unwrap();

        for (line_index, line) in fs::read_to_string(&path).unwrap().lines().enumerate() {
            let Some((test_op, args_str)) = line.trim().split_once('(') else {
                continue;
            };

            if !test_op.starts_with("TEST_") {
                continue;
            }
            let args_str = args_str.strip_suffix(")").unwrap();
            let line_number = line_index + 1;
            let file_line_number = &format!("{file_name}:{line_number}");

            match test_op {
                "TEST_RR_OP" => test_rr_op(args_str, file_line_number, map_instruction_name),
                "TEST_IMM_OP" => test_imm_op(args_str, file_line_number, map_instruction_name),
                "TEST_RD_OP" => test_rd_op(args_str, file_line_number, map_instruction_name),
                "TEST_AUIPC" => test_auipc_op(args_str, file_line_number, map_instruction_name),
                "TEST_BRANCH_OP" => {
                    test_branch_op(args_str, file_line_number, map_instruction_name)
                }
                "TEST_JAL_OP" => test_jal_op(args_str, file_line_number, map_instruction_name),
                "TEST_JALR_OP" => test_jalr_op(args_str, file_line_number, map_instruction_name),
                "TEST_CASE" => test_case_op(args_str, file_line_number, map_instruction_name),
                "TEST_LOAD" => test_load_op(args_str, file_line_number, map_instruction_name),
                "TEST_STORE" => test_store_op(args_str, file_line_number, map_instruction_name),
                test_op => {
                    panic!("Unsupported test operation: {}", test_op);
                }
            }

            case_count += 1;
        }
    }

    assert!(case_count > 0);
}

fn test_rr_op<I>(
    args_str: &str,
    file_line_number: &str,
    map_instruction_name: fn(&str, RunTestArg, RunTestArg, RunTestArg) -> Option<I>,
) where
    I: Instruction<Reg = Reg<u64>> + ExecutableInstruction<TestInterpreterState<I>, &'static str>,
{
    let (instruction_name, rd, rs1, rs2, expected, rs1_val, rs2_val) = parse_test_rr_op(args_str);

    let Some(instruction) = map_instruction_name(
        &instruction_name,
        RunTestArg::Reg(Reg::from_bits(rd).unwrap()),
        RunTestArg::Reg(Reg::from_bits(rs1).unwrap()),
        RunTestArg::Reg(Reg::from_bits(rs2).unwrap()),
    ) else {
        // Skip
        return;
    };

    let mut state = initialize_state([instruction]);

    state.regs.write(Reg::from_bits(rs1).unwrap(), rs1_val);
    state.regs.write(Reg::from_bits(rs2).unwrap(), rs2_val);

    if let Err(error) = execute(&mut state) {
        panic!(
            "Execution error at {file_line_number} {instruction_name} \
            rs1=x{rs1}({rs1_val:#x}) rs2=x{rs2}({rs2_val:#x}): {error}",
        );
    }

    let actual = state.regs.read(Reg::from_bits(rd).unwrap());
    if actual != expected {
        panic!(
            "Unexpected result at {file_line_number} {instruction_name} \
            rs1=x{rs1}({rs1_val:#x}) rs2=x{rs2}({rs2_val:#x}) \
            expected={expected:#x} actual={actual:#x}",
        );
    }
}

fn parse_test_rr_op(args_str: &str) -> (String, u8, u8, u8, u64, u64, u64) {
    let mut args = args_str.split(',').map(|s| s.trim());

    // General structure (10 arguments total in the official suite):
    // TEST_RR_OP(
    //   Instruction name
    //   Destination register (rd)
    //   Source register 1 (rs1)
    //   Source register 2 (rs2)
    //   Expected result (precomputed for the inputs)
    //   Value to load into rs1
    //   Value to load into rs2
    //   Signature base register (only used in the official tests runner)
    //   Byte offset into the signature (only used in the official tests runner)
    //   Temporary/scratch register (only used in the official tests runner)
    // )

    let instruction_name = args.next().unwrap().to_lowercase();

    let rd = parse_register(args.next().unwrap());
    let rs1 = parse_register(args.next().unwrap());
    let rs2 = parse_register(args.next().unwrap());

    let expected = parse_value(args.next().unwrap());
    let rs1_val = parse_value(args.next().unwrap());
    let rs2_val = parse_value(args.next().unwrap());

    // 3 more elements remaining, only used in the official tests runner
    assert_eq!(args.count(), 3);

    (instruction_name, rd, rs1, rs2, expected, rs1_val, rs2_val)
}

fn test_imm_op<I>(
    args_str: &str,
    file_line_number: &str,
    map_instruction_name: fn(&str, RunTestArg, RunTestArg, RunTestArg) -> Option<I>,
) where
    I: Instruction<Reg = Reg<u64>> + ExecutableInstruction<TestInterpreterState<I>, &'static str>,
{
    let (instruction_name, rd, rs1, imm, expected, rs1_val) = parse_test_imm_op(args_str);

    let Some(instruction) = map_instruction_name(
        &instruction_name,
        RunTestArg::Reg(Reg::from_bits(rd).unwrap()),
        RunTestArg::Reg(Reg::from_bits(rs1).unwrap()),
        RunTestArg::I16(imm),
    ) else {
        // Skip
        return;
    };

    let mut state = initialize_state([instruction]);

    state.regs.write(Reg::from_bits(rs1).unwrap(), rs1_val);

    if let Err(error) = execute(&mut state) {
        panic!(
            "Execution error at {file_line_number} {instruction_name} \
            rs1=x{rs1}({rs1_val:#x}) imm={imm} : {error}",
        );
    }

    let actual = state.regs.read(Reg::from_bits(rd).unwrap());
    if actual != expected {
        panic!(
            "Unexpected result at {file_line_number} {instruction_name} \
            rs1=x{rs1}({rs1_val:#x}) imm={imm} \
            expected={expected:#x} actual={actual:#x}",
        );
    }
}

fn parse_test_imm_op(args_str: &str) -> (String, u8, u8, i16, u64, u64) {
    let mut args = args_str.split(',').map(|s| s.trim());

    // General structure (9 arguments total in the official suite):
    // TEST_IMM_OP(
    //   Instruction name
    //   Destination register (rd)
    //   Source register 1 (rs1)
    //   Expected result (precomputed for the inputs)
    //   Value to load into rs1
    //   Immediate value (possibly signed, within 12-bit range)
    //   Signature base register (only used in the official tests runner)
    //   Byte offset into the signature (only used in the official tests runner)
    //   Temporary/scratch register (only used in the official tests runner)
    // )

    let instruction_name = args.next().unwrap().to_lowercase();

    let rd = parse_register(args.next().unwrap());
    let rs1 = parse_register(args.next().unwrap());

    let expected = parse_value(args.next().unwrap());
    let rs1_val = parse_value(args.next().unwrap());
    let imm = parse_imm_12_bits(args.next().unwrap());

    // 3 more elements remaining, only used in the official tests runner
    assert_eq!(args.count(), 3);

    (instruction_name, rd, rs1, imm, expected, rs1_val)
}

fn test_rd_op<I>(
    args_str: &str,
    file_line_number: &str,
    map_instruction_name: fn(&str, RunTestArg, RunTestArg, RunTestArg) -> Option<I>,
) where
    I: Instruction<Reg = Reg<u64>> + ExecutableInstruction<TestInterpreterState<I>, &'static str>,
{
    let (instruction_name, rd, rs1, expected, rs1_val) = parse_test_rd_op(args_str);

    let Some(instruction) = map_instruction_name(
        &instruction_name,
        RunTestArg::Reg(Reg::from_bits(rd).unwrap()),
        RunTestArg::Reg(Reg::from_bits(rs1).unwrap()),
        // A dummy third arg – not used for single-source ops
        RunTestArg::Dummy,
    ) else {
        // Skip
        return;
    };

    let mut state = initialize_state([instruction]);

    state.regs.write(Reg::from_bits(rs1).unwrap(), rs1_val);

    if let Err(error) = execute(&mut state) {
        panic!(
            "Execution error at {file_line_number} {instruction_name} \
            rs1=x{rs1}({rs1_val:#x}): {error}",
        );
    }

    let actual = state.regs.read(Reg::from_bits(rd).unwrap());
    if actual != expected {
        panic!(
            "Unexpected result at {file_line_number} {instruction_name} \
            rs1=x{rs1}({rs1_val:#x}) \
            expected={expected:#x} actual={actual:#x}",
        );
    }
}

fn parse_test_rd_op(args_str: &str) -> (String, u8, u8, u64, u64) {
    let mut args = args_str.split(',').map(|s| s.trim());

    // Expected structure (based on "single operand" macro added for extensions like Zcb/Zk):
    // TEST_RD_OP(
    //   Instruction name
    //   Destination register (rd)
    //   Source register (rs1)
    //   Expected result (precomputed for the inputs)
    //   Value to load into rs1
    //   Signature base register (only used in the official tests runner)
    //   Byte offset into the signature (only used in the official tests runner)
    //   Temporary/scratch register (only used in the official tests runner)
    // )

    let instruction_name = args.next().unwrap().to_lowercase();

    let rd = parse_register(args.next().unwrap());
    let rs1 = parse_register(args.next().unwrap());

    let expected = parse_value(args.next().unwrap());
    let rs1_val = parse_value(args.next().unwrap());

    // 3 more elements remaining, only used in the official tests runner
    assert_eq!(args.count(), 3);

    (instruction_name, rd, rs1, expected, rs1_val)
}

fn test_auipc_op<I>(
    args_str: &str,
    file_line_number: &str,
    map_instruction_name: fn(&str, RunTestArg, RunTestArg, RunTestArg) -> Option<I>,
) where
    I: Instruction<Reg = Reg<u64>> + ExecutableInstruction<TestInterpreterState<I>, &'static str>,
{
    let (instruction_name, rd, expected, raw_imm) = parse_test_auipc_op(args_str);

    // The decoder extracts (instr & 0xffff_f000).cast_signed() → already-shifted sign-extended i32
    // The test's third arg is exactly that value (as u64 bit pattern), and it always fits in i32
    let imm_i32 = expected as i32;
    // Display the conventional unsigned shifted immediate
    let displayed_imm = (raw_imm as u64) << 12;

    let Some(instruction) = map_instruction_name(
        &instruction_name,
        RunTestArg::Reg(Reg::from_bits(rd).unwrap()),
        RunTestArg::I32(imm_i32),
        // A dummy third arg – not used for U-type ops
        RunTestArg::Dummy,
    ) else {
        // Skip
        return;
    };

    let mut state = initialize_state([instruction]);

    if let Err(error) = execute(&mut state) {
        panic!(
            "Execution error at {file_line_number} {instruction_name} \
            rd=x{rd} imm={displayed_imm:#x}: {error}",
        );
    }

    let actual = state.regs.read(Reg::from_bits(rd).unwrap());
    if actual != expected {
        panic!(
            "Unexpected result at {file_line_number} {instruction_name} \
            rd=x{rd} imm={displayed_imm:#x} \
            expected={expected:#x} actual={actual:#x}",
        );
    }
}

fn parse_test_auipc_op(args_str: &str) -> (String, u8, u64, u32) {
    let mut args = args_str.split(',').map(|s| s.trim());
    // General structure (7 arguments total in the official suite):
    // TEST_AUIPC(
    //   Instruction name
    //   Destination register (rd)
    //   Expected result (sext(raw_imm << 12), assuming PC=0)
    //   Raw immediate value (20-bit unsigned field, always printed positive)
    //   Signature base register (only used in the official tests runner)
    //   Byte offset into the signature (only used in the official tests runner)
    //   Temporary/scratch register (only used in the official tests runner)
    // )
    let instruction_name = args.next().unwrap().to_lowercase();
    let rd = parse_register(args.next().unwrap());
    let expected = parse_value(args.next().unwrap());
    let raw_imm = parse_raw_auipc_imm(args.next().unwrap());
    // 3 more elements remaining, only used in the official tests runner
    assert_eq!(args.count(), 3);
    (instruction_name, rd, expected, raw_imm)
}

fn test_branch_op<I>(
    args_str: &str,
    file_line_number: &str,
    map_instruction_name: fn(&str, RunTestArg, RunTestArg, RunTestArg) -> Option<I>,
) where
    I: Instruction<Reg = Reg<u64>> + ExecutableInstruction<TestInterpreterState<I>, &'static str>,
{
    let (instruction_name, rs1, rs2, rs1_val, rs2_val, imm_signed) = parse_test_branch_op(args_str);

    let imm_i32 = imm_signed as i32;

    let Some(instruction) = map_instruction_name(
        &instruction_name,
        RunTestArg::Reg(Reg::from_bits(rs1).unwrap()),
        RunTestArg::Reg(Reg::from_bits(rs2).unwrap()),
        RunTestArg::I32(imm_i32),
    ) else {
        // Skip
        return;
    };

    let mut state = initialize_state(vec![instruction]);

    state.regs.write(Reg::from_bits(rs1).unwrap(), rs1_val);
    state.regs.write(Reg::from_bits(rs2).unwrap(), rs2_val);

    let fetch_result = match state
        .instruction_fetcher
        .fetch_instruction(&mut state.memory)
    {
        Ok(result) => result,
        Err(error) => {
            panic!("Fetch error at {file_line_number}: {error}");
        }
    };

    let FetchInstructionResult::Instruction(instruction) = fetch_result else {
        panic!("Unexpected control flow during fetch at {file_line_number}");
    };

    match instruction.execute(&mut state) {
        Ok(result) => result.continue_ok().unwrap(),
        Err(error) => {
            panic!(
                "Execution error at {file_line_number} {instruction_name} \
                rs1=x{rs1}({rs1_val:#x}) rs2=x{rs2}({rs2_val:#x}) imm={imm_signed}: {error}"
            )
        }
    }

    let final_pc = state.instruction_fetcher.get_pc();

    // TODO: This is a hack, but the simplest one out of many options. Would be nice to not hardcode
    //  these conditions though.
    let should_take = match instruction_name.as_str() {
        "beq" => rs1_val == rs2_val,
        "bne" => rs1_val != rs2_val,
        "blt" => rs1_val.cast_signed() < rs2_val.cast_signed(),
        "bge" => rs1_val.cast_signed() >= rs2_val.cast_signed(),
        "bltu" => rs1_val < rs2_val,
        "bgeu" => rs1_val >= rs2_val,
        _ => {
            panic!("Unsupported branch instruction: {instruction_name}");
        }
    };

    let expected_pc = if should_take {
        // `old_pc` after fetch is effectively 0, add the signed byte offset (wrapping semantics)
        imm_signed.cast_unsigned()
    } else {
        u64::from(instruction.size())
    };

    if final_pc != expected_pc {
        panic!(
            "Unexpected branch behavior at {file_line_number} {instruction_name} \
            rs1=x{rs1}({rs1_val:#x}) rs2=x{rs2}({rs2_val:#x}) imm={imm_signed} \
            expected {}taken, final_pc={final_pc:#x} expected_pc={expected_pc:#x}",
            if should_take { "" } else { "not " },
        );
    }
}

fn parse_test_branch_op(args_str: &str) -> (String, u8, u8, u64, u64, i64) {
    let mut args = args_str.split(',').map(|s| s.trim());

    // General structure (10 arguments total in the official suite):
    // TEST_BRANCH_OP(
    //   Instruction name
    //   Temporary/scratch register (rd field, ignored)
    //   Source register 1 (rs1)
    //   Source register 2 (rs2)
    //   Value to load into rs1 (bit pattern)
    //   Value to load into rs2 (bit pattern)
    //   Signed byte offset (may be negative, e.g., -0x20 or 0x400)
    //   Branch target label (e.g., 1b or 3f, ignored for expectation)
    //   Signature base register (only used in the official tests runner)
    //   Byte offset into the signature (only used in the official tests runner)
    //   Align/flag (usually 0, only used in the official tests runner)
    // )
    let instruction_name = args.next().unwrap().to_lowercase();

    // Skip temporary register
    parse_register(args.next().unwrap());

    let rs1 = parse_register(args.next().unwrap());
    let rs2 = parse_register(args.next().unwrap());

    let rs1_val = parse_value(args.next().unwrap());
    let rs2_val = parse_value(args.next().unwrap());

    let imm_str = args.next().unwrap();
    let imm_signed = parse_branch_imm(imm_str);

    // 4 more elements remaining, only used in the official tests runner
    assert_eq!(args.count(), 4);

    (instruction_name, rs1, rs2, rs1_val, rs2_val, imm_signed)
}

fn test_jal_op<I>(
    args_str: &str,
    file_line_number: &str,
    map_instruction_name: fn(&str, RunTestArg, RunTestArg, RunTestArg) -> Option<I>,
) where
    I: Instruction<Reg = Reg<u64>> + ExecutableInstruction<TestInterpreterState<I>, &'static str>,
{
    let (rd, encoded) = parse_test_jal_op(args_str);

    let byte_offset = compute_jal_byte_offset(encoded);

    let Some(instruction) = map_instruction_name(
        "jal",
        RunTestArg::Reg(Reg::from_bits(rd).unwrap()),
        RunTestArg::I32(byte_offset),
        RunTestArg::Dummy,
    ) else {
        panic!("Failed to map `jal` instruction at {file_line_number}");
    };

    let mut state = initialize_state(vec![instruction]);

    let fetch_result = match state
        .instruction_fetcher
        .fetch_instruction(&mut state.memory)
    {
        Ok(result) => result,
        Err(error) => {
            panic!("Fetch error at {file_line_number}: {error}");
        }
    };

    let FetchInstructionResult::Instruction(instruction) = fetch_result else {
        panic!("Unexpected control flow during fetch at {file_line_number}");
    };

    match instruction.execute(&mut state) {
        Ok(result) => result.continue_ok().unwrap(),
        Err(error) => {
            panic!(
                "Execution error at {file_line_number} jal rd=x{rd} encoded_imm={encoded:#x}: {error}"
            );
        }
    }

    let final_pc = state.instruction_fetcher.get_pc();
    let expected_target = u64::from(byte_offset.cast_unsigned());

    if final_pc != expected_target {
        panic!(
            "Unexpected target PC at {file_line_number} jal rd=x{rd} encoded_imm={encoded:#x} \
            expected_pc={expected_target:#x} actual_pc={final_pc:#x}"
        );
    }

    let rd_reg = Reg::from_bits(rd).unwrap();
    let expected_link = if rd == 0 { 0 } else { 4 };
    let actual_link = if rd == 0 { 0 } else { state.regs.read(rd_reg) };

    if actual_link != expected_link {
        panic!(
            "Unexpected link value at {file_line_number} jal rd=x{rd} encoded_imm={encoded:#x} \
            expected_link={expected_link:#x} actual={actual_link:#x}"
        );
    }
}

fn parse_test_jal_op(args_str: &str) -> (u8, u32) {
    let mut args = args_str.split(',').map(|s| s.trim());

    // General structure (7 arguments total in the official suite):
    // TEST_JAL_OP(
    //   Temporary/scratch register (ignored)
    //   Destination register (rd)
    //   Raw encoded immediate (20-bit field value)
    //   Target label (only used in the official tests runner)
    //   Signature base register (only used in the official tests runner)
    //   Byte offset into the signature (only used in the official tests runner)
    //   Align/flag (only used in the official tests runner)
    // )

    // Skip temporary register
    parse_register(args.next().unwrap());

    let rd = parse_register(args.next().unwrap());
    let encoded = parse_jal_encoded_imm(args.next().unwrap());

    // 4 more elements remaining, only used in the official tests runner
    assert_eq!(args.count(), 4);

    (rd, encoded)
}

fn test_jalr_op<I>(
    args_str: &str,
    file_line_number: &str,
    map_instruction_name: fn(&str, RunTestArg, RunTestArg, RunTestArg) -> Option<I>,
) where
    I: Instruction<Reg = Reg<u64>> + ExecutableInstruction<TestInterpreterState<I>, &'static str>,
{
    let (rd, rs1, imm) = parse_test_jalr_op(args_str);

    let Some(instruction) = map_instruction_name(
        "jalr",
        RunTestArg::Reg(Reg::from_bits(rd).unwrap()),
        RunTestArg::Reg(Reg::from_bits(rs1).unwrap()),
        RunTestArg::I16(imm),
    ) else {
        panic!("Failed to map `jalr` instruction at {file_line_number}");
    };

    let mut state = initialize_state(vec![instruction]);

    // rs1 is assumed to be 0 initially (as in the arch test environment for these coverpoints)

    let fetch_result = match state
        .instruction_fetcher
        .fetch_instruction(&mut state.memory)
    {
        Ok(result) => result,
        Err(error) => {
            panic!("Fetch error at {file_line_number}: {error}");
        }
    };

    let FetchInstructionResult::Instruction(instruction) = fetch_result else {
        panic!("Unexpected control flow during fetch at {file_line_number}");
    };

    match instruction.execute(&mut state) {
        Ok(result) => result.continue_ok().unwrap(),
        Err(error) => {
            panic!(
                "Execution error at {file_line_number} jalr rd=x{rd} rs1=x{rs1} imm={imm}: {error}"
            );
        }
    }

    let final_pc = state.instruction_fetcher.get_pc();

    // Expected target: (rs1=0 + sext(imm)) & ~1
    let expected_target = i64::from(imm).cast_unsigned() & !1u64;

    if final_pc != expected_target {
        panic!(
            "Unexpected target PC at {file_line_number} jalr rd=x{rd} rs1=x{rs1} imm={imm} \
            expected_pc={expected_target:#x} actual_pc={final_pc:#x}"
        );
    }

    // Link register check
    let rd_reg = Reg::from_bits(rd).unwrap();
    let expected_link = if rd == 0 { 0 } else { 4 };
    let actual_link = if rd == 0 { 0 } else { state.regs.read(rd_reg) };

    if actual_link != expected_link {
        panic!(
            "Unexpected link value at {file_line_number} jalr rd=x{rd} rs1=x{rs1} imm={imm} \
            expected_link={expected_link:#x} actual={actual_link:#x}"
        );
    }
}

fn parse_test_jalr_op(args_str: &str) -> (u8, u8, i16) {
    let mut args = args_str.split(',').map(|s| s.trim());

    // General structure (7 arguments total in the official suite):
    // TEST_JALR_OP(
    //   Temporary/scratch register (ignored)
    //   Destination register (rd)
    //   Source register (rs1, initially 0)
    //   Immediate value (signed 12-bit)
    //   Signature base register (only used in the official tests runner)
    //   Byte offset into the signature (only used in the official tests runner)
    //   Align/flag (only used in the official tests runner)
    // )

    // Skip temporary register
    parse_register(args.next().unwrap());

    let rd = parse_register(args.next().unwrap());
    let rs1 = parse_register(args.next().unwrap());
    let imm = parse_jalr_imm(args.next().unwrap()) as i16;

    // 3 more elements remaining, only used in the official tests runner
    assert_eq!(args.count(), 3);

    (rd, rs1, imm)
}

fn test_case_op<I>(
    args_str: &str,
    file_line_number: &str,
    map_instruction_name: fn(&str, RunTestArg, RunTestArg, RunTestArg) -> Option<I>,
) where
    I: Instruction<Reg = Reg<u64>> + ExecutableInstruction<TestInterpreterState<I>, &'static str>,
{
    let (instruction_name, rd, expected, raw_imm) = parse_test_case(args_str);

    // The full signed offset fits in i32 (as in TEST_AUIPC)
    let imm_i32 = expected as i32;
    // Display the conventional unsigned shifted immediate
    let displayed_imm = (raw_imm as u64) << 12;

    let Some(instruction) = map_instruction_name(
        &instruction_name,
        RunTestArg::Reg(Reg::from_bits(rd).unwrap()),
        RunTestArg::I32(imm_i32),
        // A dummy third arg – not used for U-type ops
        RunTestArg::Dummy,
    ) else {
        // Skip
        return;
    };

    let mut state = initialize_state([instruction]);

    if let Err(error) = execute(&mut state) {
        panic!(
            "Execution error at {file_line_number} {instruction_name} \
            rd=x{rd} imm={displayed_imm:#x}: {error}",
        );
    }

    let actual = state.regs.read(Reg::from_bits(rd).unwrap());
    if actual != expected {
        panic!(
            "Unexpected result at {file_line_number} {instruction_name} \
            rd=x{rd} imm={displayed_imm:#x} expected={expected:#x} actual={actual:#x}",
        );
    }
}

fn parse_test_case(args_str: &str) -> (String, u8, u64, u32) {
    let mut args = args_str.split(", ").map(|s| s.trim());

    // General structure (6 arguments total in the official suite):
    // TEST_CASE(
    //   Temporary/scratch register (ignored)
    //   Destination register (rd)
    //   Expected result (sext(raw_imm << 12))
    //   Signature base register (only used in the official tests runner)
    //   Byte offset into the signature (only used in the official tests runner)
    //   `lui rd,raw_imm` the actual instruction (raw_imm is 20-bit unsigned, with 0x prefix)
    // )

    // Skip temporary register
    parse_register(args.next().unwrap());

    let rd = parse_register(args.next().unwrap());

    let expected = parse_value(args.next().unwrap());

    // Signature base register (ignored)
    parse_register(args.next().unwrap());
    // Byte offset into the signature (ignored)
    args.next().unwrap();

    let code_str = args.next().unwrap();

    // No more arguments allowed
    assert_eq!(args.count(), 0);

    let mut code_parts = code_str.split_whitespace();

    let instruction_name = code_parts.next().unwrap().to_lowercase();

    let Some(rd_imm_str) = code_parts.next() else {
        panic!("Missing `rd,imm` part in TEST_CASE code: {args_str}");
    };

    if code_parts.next().is_some() {
        panic!("Extra content in TEST_CASE code");
    }

    let Some((rd_code_str, imm_str)) = rd_imm_str.split_once(',') else {
        panic!("Malformed `rd,imm` part in TEST_CASE code: {args_str}");
    };

    let rd_code = parse_register(rd_code_str);
    if rd != rd_code {
        panic!("`rd` mismatch between arguments and code in TEST_CASE: {args_str}");
    }

    let raw_imm = parse_raw_auipc_imm(imm_str);

    (instruction_name, rd, expected, raw_imm)
}

fn test_load_op<I>(
    args_str: &str,
    file_line_number: &str,
    map_instruction_name: fn(&str, RunTestArg, RunTestArg, RunTestArg) -> Option<I>,
) where
    I: Instruction<Reg = Reg<u64>> + ExecutableInstruction<TestInterpreterState<I>, &'static str>,
{
    let (instruction_name, rd, rs1, imm) = parse_test_load_op(args_str);
    let imm_display = if imm >= 0 {
        format!("0x{imm:x}")
    } else {
        format!("-0x{:x}", imm.abs())
    };

    let Some(instruction) = map_instruction_name(
        &instruction_name,
        RunTestArg::Reg(Reg::from_bits(rd).unwrap()),
        RunTestArg::Reg(Reg::from_bits(rs1).unwrap()),
        RunTestArg::I16(imm),
    ) else {
        panic!("Unknown load instruction: {}", instruction_name);
    };

    let mut state = initialize_state([instruction]);

    // Standard `rvtest_data` from the compliance suite: 16 bytes of repeating 0xbabecafe
    // (little-endian)
    // 4096, safe spot: allows -2048 offset -> ea=2048, no underflow/wrap
    const RVTEST_DATA_BASE: u64 = 0x1000;
    const PATTERN_U32: u32 = 0xbabecafe;
    for i in 0..4 {
        state
            .memory
            .write(RVTEST_DATA_BASE + i * 4, PATTERN_U32)
            .unwrap();
    }

    // Mimic the test environment: rs1 points to the start of `rvtest_data`
    state
        .regs
        .write(Reg::from_bits(rs1).unwrap(), RVTEST_DATA_BASE);

    if let Err(error) = execute(&mut state) {
        panic!(
            "Execution error at {file_line_number} {instruction_name} \
            rs1=x{rs1}({RVTEST_DATA_BASE:#x}) imm={imm_display}: {error}",
        );
    }

    let actual = state.regs.read(Reg::from_bits(rd).unwrap());

    // Compute effective address (matches RISC-V signed offset addition, no wrap in this range)
    let offset = imm as i64;
    let effective_addr = if offset >= 0 {
        RVTEST_DATA_BASE + offset.cast_unsigned()
    } else {
        RVTEST_DATA_BASE - offset.unsigned_abs()
    };

    // TODO: This is a hack, but the simplest one out of many options. Would be nice to not hardcode
    //  these conditions though.
    let expected = match instruction_name.as_str() {
        "ld" => state.memory.read::<u64>(effective_addr).unwrap(),
        "lw" => {
            let v = state.memory.read::<i32>(effective_addr).unwrap();
            i64::from(v).cast_unsigned()
        }
        "lwu" => state.memory.read::<u32>(effective_addr).unwrap() as u64,
        "lh" => {
            let v = state.memory.read::<i16>(effective_addr).unwrap();
            i64::from(v).cast_unsigned()
        }
        "lhu" => state.memory.read::<u16>(effective_addr).unwrap() as u64,
        "lb" => {
            let v = state.memory.read::<i8>(effective_addr).unwrap();
            i64::from(v).cast_unsigned()
        }
        "lbu" => u64::from(state.memory.read::<u8>(effective_addr).unwrap()),
        _ => {
            panic!("Unsupported load instruction: {instruction_name}");
        }
    };

    if actual != expected {
        panic!(
            "Unexpected loaded value at {file_line_number} {instruction_name} \
            rs1=x{rs1}({RVTEST_DATA_BASE:#x}) imm={imm_display} ea={effective_addr:#x} \
            expected={expected:#x} actual={actual:#x}",
        );
    }
}

fn parse_test_load_op(args_str: &str) -> (String, u8, u8, i16) {
    let mut args = args_str.split(',').map(|s| s.trim());

    // General structure (9 arguments total in the official suite):
    // TEST_LOAD(
    //   Signature base register (only used in the official tests runner)
    //   Temporary/scratch register (only used in the official tests runner)
    //   Constant flag (only used in the official tests runner)
    //   Source register 1 (rs1, base address register)
    //   Destination register (rd)
    //   Immediate value (signed 12-bit, printed in hex, possibly negative, e.g., -0x800)
    //   Signature offset / computed value (only used in the official tests runner)
    //   Instruction name
    //   Constant flag (only used in the official tests runner)
    // )

    // Ignore signature-related and flag arguments
    parse_register(args.next().unwrap());
    parse_register(args.next().unwrap());
    args.next().unwrap();

    let rs1 = parse_register(args.next().unwrap());
    let rd = parse_register(args.next().unwrap());

    let imm = parse_imm_12_bits(args.next().unwrap());

    // Ignore signature offset / computed value
    args.next().unwrap();

    let instruction_name = args.next().unwrap().to_lowercase();

    // 1 more element remaining, only used in the official tests runner
    assert_eq!(args.count(), 1);

    (instruction_name, rd, rs1, imm)
}

fn test_store_op<I>(
    args_str: &str,
    file_line_number: &str,
    map_instruction_name: fn(&str, RunTestArg, RunTestArg, RunTestArg) -> Option<I>,
) where
    I: Instruction<Reg = Reg<u64>> + ExecutableInstruction<TestInterpreterState<I>, &'static str>,
{
    let (instruction_name, rs1, rs2, imm, store_val) = parse_test_store_op(args_str);
    let imm_display = if imm >= 0 {
        format!("0x{imm:x}")
    } else {
        format!("-0x{:x}", imm.abs())
    };

    let Some(instruction) = map_instruction_name(
        &instruction_name,
        RunTestArg::Reg(Reg::from_bits(rs2).unwrap()),
        RunTestArg::Reg(Reg::from_bits(rs1).unwrap()),
        RunTestArg::I16(imm),
    ) else {
        panic!("Unknown store instruction: {}", instruction_name);
    };

    let mut state = initialize_state([instruction]);

    // Standard `rvtest_data` base from the compliance suite, placed safely to allow negative
    // offsets
    const RVTEST_DATA_BASE: u64 = 0x1000;

    // Mimic the test environment: rs1 points to the start of `rvtest_data`
    state
        .regs
        .write(Reg::from_bits(rs1).unwrap(), RVTEST_DATA_BASE);
    state.regs.write(Reg::from_bits(rs2).unwrap(), store_val);

    if let Err(error) = execute(&mut state) {
        panic!(
            "Execution error at {file_line_number} {instruction_name} \
            rs1=x{rs1}({RVTEST_DATA_BASE:#x}) rs2=x{rs2}({store_val:#x}) imm={imm_display}: {error}",
        );
    }

    // Compute effective address (matches RISC-V signed offset addition, safe in this range)
    let offset = i64::from(imm).cast_unsigned();
    let effective_addr = RVTEST_DATA_BASE.wrapping_add(offset);

    // TODO: This is a hack, but the simplest one out of many options. Would be nice to not hardcode
    //  these conditions though.
    // Expected stored value (low bits only, no sign-extension on store)
    let expected = match instruction_name.as_str() {
        "sb" => u64::from(store_val as u8),
        "sh" => u64::from(store_val as u16),
        "sw" => u64::from(store_val as u32),
        "sd" => store_val,
        _ => {
            panic!("Unsupported store instruction: {instruction_name}");
        }
    };

    // TODO: This is a hack, but the simplest one out of many options. Would be nice to not hardcode
    //  these conditions though.
    // Actual value read back from memory (memory starts zero-filled)
    let actual = match instruction_name.as_str() {
        "sb" => u64::from(state.memory.read::<u8>(effective_addr).unwrap()),
        "sh" => u64::from(state.memory.read::<u16>(effective_addr).unwrap()),
        "sw" => u64::from(state.memory.read::<u32>(effective_addr).unwrap()),
        "sd" => state.memory.read::<u64>(effective_addr).unwrap(),
        _ => {
            panic!("Unsupported store instruction: {instruction_name}");
        }
    };

    if actual != expected {
        panic!(
            "Unexpected stored value at {file_line_number} {instruction_name} \
            rs1=x{rs1}({RVTEST_DATA_BASE:#x}) rs2=x{rs2}({store_val:#x}) imm={imm_display} \
            ea={effective_addr:#x} expected={expected:#x} actual={actual:#x}",
        );
    }
}

fn parse_test_store_op(args_str: &str) -> (String, u8, u8, i16, u64) {
    let mut args = args_str.split(',').map(|s| s.trim());

    // General structure (10 arguments total in the official suite):
    // TEST_STORE(
    //   Signature base register (only used in the official tests runner)
    //   Temporary/scratch register (only used in the official tests runner)
    //   Constant flag (only used in the official tests runner)
    //   Source register 1 (rs1, base address register)
    //   Source register 2 (rs2, data register)
    //   Value to load into rs2
    //   Immediate value (signed 12-bit)
    //   Signature offset / computed value (only used in the official tests runner)
    //   Instruction name
    //   Constant flag (only used in the official tests runner)
    // )

    // Ignore signature-related and flag arguments
    parse_register(args.next().unwrap());
    parse_register(args.next().unwrap());
    args.next().unwrap();

    let rs1 = parse_register(args.next().unwrap());
    let rs2 = parse_register(args.next().unwrap());

    let store_val = parse_value(args.next().unwrap());
    let imm = parse_imm_12_bits(args.next().unwrap());

    // Ignore signature offset / computed value
    args.next().unwrap();

    let instruction_name = args.next().unwrap().to_lowercase();

    // 1 more element remaining, only used in the official tests runner
    assert_eq!(args.count(), 1);

    (instruction_name, rs1, rs2, imm, store_val)
}

fn parse_imm_12_bits(s: &str) -> i16 {
    let s = s.trim();
    if s == "0" {
        return 0;
    }

    let (neg, s) = match s.strip_prefix('-') {
        Some(stripped) => (true, stripped),
        None => (false, s),
    };

    let imm = u16::from_str_radix(s.strip_prefix("0x").unwrap(), 16)
        .unwrap()
        .cast_signed();

    let raw_value = if neg { -imm } else { imm };
    // Sign extension is expected to be done in the decoder
    let value = raw_value.cast_unsigned();
    (value << 4).cast_signed() >> 4
}

fn parse_branch_imm(s: &str) -> i64 {
    let s = s.trim();
    if s == "0" {
        return 0;
    }

    let (neg, s) = match s.strip_prefix('-') {
        Some(stripped) => (true, stripped),
        None => (false, s),
    };

    let pos_str = s.strip_prefix("0x").unwrap();
    let pos = u64::from_str_radix(pos_str, 16).unwrap().cast_signed();

    if neg { -pos } else { pos }
}

fn parse_raw_auipc_imm(s: &str) -> u32 {
    let s = s.trim();
    if s == "0" {
        0
    } else {
        u32::from_str_radix(s.strip_prefix("0x").unwrap(), 16).unwrap()
    }
}

fn parse_jal_encoded_imm(s: &str) -> u32 {
    let s = s.trim();
    if s == "0" {
        0
    } else {
        u32::from_str_radix(s.strip_prefix("0x").unwrap(), 16).unwrap()
    }
}

fn compute_jal_byte_offset(encoded: u32) -> i32 {
    // Replicate the sign-extension behavior for the 20-bit J-type immediate
    // encoded: the raw 20-bit immediate field value (as passed in the test macro)
    // First, sign-extend from bit 19 to 32 bits
    let se_encoded = ((encoded.cast_signed()) << 12) >> 12;
    // Then shift left by 1 to get the signed byte offset
    se_encoded << 1
}

fn parse_jalr_imm(s: &str) -> i64 {
    let s = s.trim();
    if s == "0" {
        return 0;
    }

    let (neg, s) = match s.strip_prefix('-') {
        Some(stripped) => (true, stripped),
        None => (false, s),
    };

    let hex_part = s.strip_prefix("0x").unwrap();
    let mag = u64::from_str_radix(hex_part, 16).unwrap().cast_signed();

    if neg { -mag } else { mag }
}

fn parse_register(s: &str) -> u8 {
    s.strip_prefix('x').unwrap().parse().unwrap()
}

fn parse_value(s: &str) -> u64 {
    if s == "0" {
        return 0;
    }

    let (neg, s) = match s.strip_prefix('-') {
        Some(s) => (true, s),
        None => (false, s),
    };

    let value = u64::from_str_radix(s.strip_prefix("0x").unwrap(), 16).unwrap();
    if neg { value.wrapping_neg() } else { value }
}
