extern crate alloc;

use crate::basic::{BasicInterpreterState, BasicRegisters};
use crate::v::vector_registers::{
    VectorRegisterFile, VectorRegisters, VectorRegistersBase, VectorRegistersExt,
};
use crate::{
    Address, BasicInt, CsrError, Csrs, ExecutableInstruction, ExecutableInstructionCsr,
    ExecutionError, FetchInstructionResult, InstructionFetcher, ProgramCounter,
    ProgramCounterError, RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands,
    SystemInstructionHandler, VirtualMemory, VirtualMemoryError,
};
use ab_riscv_primitives::prelude::*;
use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;
use core::ops::ControlFlow;

pub(crate) const TEST_BASE_ADDR: u64 = 0x1000;
const TRAP_ADDRESS: u64 = 0;
/// Zve64x element width
const ZVE64X_ELEN: u32 = 64;
/// VLEN in bits for the test vector register file
const TEST_VLEN: u32 = 128;
/// VLEN in bytes
const TEST_VLENB: usize = (TEST_VLEN / u8::BITS) as usize;

/// Simple test memory implementation
pub(crate) struct TestMemory {
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
            .ok_or(VirtualMemoryError::OutOfBoundsRead { address })?;

        if offset.saturating_add(size_of::<T>() as u64) > self.data.len() as u64 {
            return Err(VirtualMemoryError::OutOfBoundsRead { address });
        }

        // SAFETY: Only reading basic integers from initialized memory
        unsafe {
            Ok(self
                .data
                .as_ptr()
                .cast::<T>()
                .byte_add(offset as usize)
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

    fn read_slice(&self, address: u64, len: u32) -> Result<&[u8], VirtualMemoryError> {
        let offset = address
            .checked_sub(self.base_addr)
            .ok_or(VirtualMemoryError::OutOfBoundsRead { address })?;

        if offset > self.data.len() as u64 {
            return Err(VirtualMemoryError::OutOfBoundsRead { address });
        }

        self.data
            .get(offset as usize..)
            .and_then(|data| data.get(..len as usize))
            .ok_or(VirtualMemoryError::OutOfBoundsRead { address })
    }

    fn read_slice_up_to(&self, address: u64, len: u32) -> &[u8] {
        let Some(offset) = address.checked_sub(self.base_addr) else {
            return &[];
        };

        if offset > self.data.len() as u64 {
            return &[];
        }

        let remaining = self.data.get(offset as usize..).unwrap_or_default();
        remaining.get(..len as usize).unwrap_or(remaining)
    }

    fn write<T>(&mut self, address: u64, value: T) -> Result<(), VirtualMemoryError>
    where
        T: BasicInt,
    {
        let offset = address
            .checked_sub(self.base_addr)
            .ok_or(VirtualMemoryError::OutOfBoundsWrite { address })?;

        if offset.saturating_add(size_of::<T>() as u64) > self.data.len() as u64 {
            return Err(VirtualMemoryError::OutOfBoundsWrite { address });
        }

        // SAFETY: Only writing basic integers to initialized memory
        unsafe {
            self.data
                .as_mut_ptr()
                .cast::<T>()
                .byte_add(offset as usize)
                .write_unaligned(value);
        }

        Ok(())
    }

    fn write_slice(&mut self, address: u64, data: &[u8]) -> Result<(), VirtualMemoryError> {
        let offset = address
            .checked_sub(self.base_addr)
            .ok_or(VirtualMemoryError::OutOfBoundsWrite { address })?;

        if offset > self.data.len() as u64 {
            return Err(VirtualMemoryError::OutOfBoundsWrite { address });
        }

        let len = data.len();
        self.data
            .get_mut(offset as usize..)
            .and_then(|data| data.get_mut(..len))
            .ok_or(VirtualMemoryError::OutOfBoundsWrite { address })?
            .copy_from_slice(data);

        Ok(())
    }
}

/// Custom instruction handler for tests that returns instructions from a sequence
pub(crate) struct TestInstructionFetcher<I> {
    instructions: Vec<Option<I>>,
    return_trap_address: u64,
    base_address: u64,
    pc: u64,
}

impl<I> ProgramCounter<u64, TestMemory> for TestInstructionFetcher<I>
where
    I: Instruction<Reg = Reg<u64>>,
{
    #[inline(always)]
    fn get_pc(&self) -> u64 {
        self.pc
    }

    fn set_pc(
        &mut self,
        _memory: &TestMemory,
        pc: u64,
    ) -> Result<ControlFlow<()>, ProgramCounterError<u64>> {
        self.pc = pc;

        Ok(ControlFlow::Continue(()))
    }
}

impl<I> InstructionFetcher<I, TestMemory> for TestInstructionFetcher<I>
where
    I: Instruction<Reg = Reg<u64>>,
{
    #[inline]
    fn fetch_instruction(
        &mut self,
        _memory: &TestMemory,
    ) -> Result<FetchInstructionResult<I>, ExecutionError<Address<I>>> {
        if self.pc == self.return_trap_address {
            return Ok(FetchInstructionResult::ControlFlow(ControlFlow::Break(())));
        }

        let Some(&maybe_instruction) = self
            .instructions
            .get((self.pc - self.base_address) as usize / size_of::<u16>())
        else {
            return Ok(FetchInstructionResult::ControlFlow(ControlFlow::Break(())));
        };

        let Some(instruction) = maybe_instruction else {
            return Err(ExecutionError::IllegalInstruction { address: self.pc });
        };
        self.pc += u64::from(instruction.size());

        Ok(FetchInstructionResult::Instruction(instruction))
    }
}

impl<I> TestInstructionFetcher<I> {
    /// Create a new instance
    #[inline(always)]
    fn new<Instructions>(
        instructions: Instructions,
        return_trap_address: u64,
        base_address: u64,
        pc: u64,
    ) -> Self
    where
        I: Instruction<Reg = Reg<u64>>,
        Instructions: IntoIterator<Item = I>,
    {
        Self {
            instructions: instructions
                .into_iter()
                .flat_map(|instruction| {
                    let maybe_second = match instruction.size() {
                        2 => None,
                        4 => {
                            // Intentionally trigger illegal instruction on the second half-word
                            Some(None)
                        }
                        instruction_size => {
                            panic!("Unexpected instruction size {instruction_size}");
                        }
                    };

                    [Some(instruction)].into_iter().chain(maybe_second)
                })
                .collect(),
            return_trap_address,
            base_address,
            pc,
        }
    }
}

pub(crate) struct TestInstructionHandler;

impl<Regs, I> SystemInstructionHandler<Reg<u64>, Regs, TestMemory, TestInstructionFetcher<I>>
    for TestInstructionHandler
where
    I: Instruction<Reg = Reg<u64>>,
    Regs: RegisterFile<Reg<u64>>,
{
    #[inline(always)]
    fn handle_ecall(
        &mut self,
        _regs: &mut Regs,
        _memory: &mut TestMemory,
        program_counter: &mut TestInstructionFetcher<I>,
    ) -> Result<ControlFlow<()>, ExecutionError<u64>> {
        Err(ExecutionError::EcallUnsupported {
            address: program_counter.old_pc(
                Rv64Instruction::<Reg<u64>>::Ecall {
                    rs1: Reg::Zero,
                    rs2: Reg::Zero,
                }
                .size(),
            ),
        })
    }
}

struct CsrExtState {
    privilege_level: PrivilegeLevel,
    csrs: BTreeMap<u16, u64>,
    prepare_csr_read: fn(csr_index: u16, raw_value: u64) -> Result<u64, CsrError>,
    prepare_csr_write: fn(csr_index: u16, write_value: u64) -> Result<u64, CsrError>,
}

impl Default for CsrExtState {
    #[inline(always)]
    fn default() -> Self {
        Self {
            privilege_level: PrivilegeLevel::Machine,
            csrs: BTreeMap::new(),
            prepare_csr_read: |csr_index, _| Err(CsrError::IllegalRead { csr_index }),
            prepare_csr_write: |csr_index, _| Err(CsrError::IllegalWrite { csr_index }),
        }
    }
}

struct VectorExtState {
    vregs: VectorRegisterFile<TEST_VLENB>,
    vtype: Option<Vtype<ZVE64X_ELEN, TEST_VLEN>>,
    vtype_raw: u64,
    vl: u32,
    vs_dirty_count: u32,
    vector_allowed: bool,
}

impl Default for VectorExtState {
    fn default() -> Self {
        Self {
            vregs: VectorRegisterFile::default(),
            vtype: None,
            vtype_raw: 1u64 << (u64::BITS - 1),
            vl: 0,
            vs_dirty_count: 0,
            vector_allowed: true,
        }
    }
}

pub(crate) struct ExtState {
    csr: CsrExtState,
    vector: VectorExtState,
}

impl Default for ExtState {
    #[inline(always)]
    fn default() -> Self {
        Self {
            csr: CsrExtState::default(),
            vector: VectorExtState::default(),
        }
    }
}

impl Csrs<Reg<u64>> for ExtState {
    fn privilege_level(&self) -> PrivilegeLevel {
        self.csr.privilege_level
    }

    fn read_csr(&self, csr_index: u16) -> Result<u64, CsrError> {
        self.csr
            .csrs
            .get(&csr_index)
            .copied()
            .ok_or(CsrError::IllegalRead { csr_index })
    }

    fn write_csr(&mut self, csr_index: u16, value: u64) -> Result<(), CsrError> {
        let stored_value = self
            .csr
            .csrs
            .get_mut(&csr_index)
            .ok_or(CsrError::IllegalWrite { csr_index })?;
        *stored_value = value;
        Ok(())
    }

    fn process_csr_read<I>(&self, csr_index: u16, raw_value: u64) -> Result<u64, CsrError>
    where
        I: ExecutableInstructionCsr<Self, Reg = Reg<u64>>,
    {
        (self.csr.prepare_csr_read)(csr_index, raw_value)
    }

    fn process_csr_write<I>(&mut self, csr_index: u16, write_value: u64) -> Result<u64, CsrError>
    where
        I: ExecutableInstructionCsr<Self, Reg = Reg<u64>>,
    {
        (self.csr.prepare_csr_write)(csr_index, write_value)
    }
}

impl VectorRegistersBase for ExtState {
    const ELEN: u32 = ZVE64X_ELEN;
    const VLEN: u32 = TEST_VLEN;
}

impl VectorRegisters for ExtState
where
    Self: Csrs<Reg<u64>>,
    [(); Self::ELEN as usize]:,
    [(); Self::VLEN as usize]:,
{
    fn read_vreg(&self) -> &VectorRegisterFile<{ Self::VLENB as usize }> {
        &self.vector.vregs
    }

    fn write_vreg(&mut self) -> &mut VectorRegisterFile<{ Self::VLENB as usize }> {
        &mut self.vector.vregs
    }

    fn vtype(&self) -> Option<Vtype<{ Self::ELEN }, { Self::VLEN }>> {
        self.vector.vtype
    }

    fn set_vtype(&mut self, vtype: Option<Vtype<{ Self::ELEN }, { Self::VLEN }>>) {
        self.vector.vtype = vtype;
        match vtype {
            Some(vt) => {
                self.vector.vtype_raw = vt.to_raw::<Reg<u64>>();
                self.write_csr(VCsr::Vtype as u16, self.vector.vtype_raw)
                    .expect("Implementation didn't initialize `vtype` CSR");
            }
            None => {
                // vill: bit `XLEN-1` set, rest zero
                self.vector.vtype_raw = 1u64 << (u64::BITS - 1);
                self.write_csr(VCsr::Vtype as u16, self.vector.vtype_raw)
                    .expect("Implementation didn't initialize `vtype` CSR");
            }
        }
    }

    fn vl(&self) -> u32 {
        self.vector.vl
    }

    fn set_vl(&mut self, vl: u32) {
        self.vector.vl = vl;
        self.write_csr(VCsr::Vl as u16, vl as u64)
            .expect("Implementation didn't initialize `vl` CSR");
    }

    fn vector_instructions_allowed(&self) -> bool {
        self.vector.vector_allowed
    }

    fn mark_vs_dirty(&mut self) {
        self.vector.vs_dirty_count += 1;
    }
}

impl VectorRegistersExt<Reg<u64>> for ExtState {}

impl ExtState {
    pub(crate) fn set_privilege_level(&mut self, privilege_level: PrivilegeLevel) {
        self.csr.privilege_level = privilege_level;
    }

    pub(crate) fn set_prepare_csr_read_write(
        &mut self,
        prepare_csr_read: fn(csr_index: u16, raw_value: u64) -> Result<u64, CsrError>,
        prepare_csr_write: fn(csr_index: u16, write_value: u64) -> Result<u64, CsrError>,
    ) {
        self.csr.prepare_csr_read = prepare_csr_read;
        self.csr.prepare_csr_write = prepare_csr_write;
    }

    /// Initialize a single CSR (without this attempts to read or write this `csr_index` will fail)
    pub(crate) fn init_csr(&mut self, csr_index: u16, value: u64) {
        self.csr.csrs.insert(csr_index, value);
    }

    /// Initialize all vector CSRs with default values
    pub(crate) fn init_vector_csrs(&mut self) {
        // Initialize all vector CSRs
        self.init_csr(VCsr::Vstart as u16, 0);
        self.init_csr(VCsr::Vxsat as u16, 0);
        self.init_csr(VCsr::Vxrm as u16, 0);
        self.init_csr(VCsr::Vcsr as u16, 0);
        self.init_csr(VCsr::Vl as u16, 0);
        self.init_csr(VCsr::Vtype as u16, 1u64 << (u64::BITS - 1));
        self.init_csr(VCsr::Vlenb as u16, u64::from(Self::VLENB));
        // Fill them with default values
        self.initialize_vector_state();
    }

    /// Configure whether vector instructions are allowed
    pub(crate) fn set_vector_allowed(&mut self, vector_allowed: bool) {
        self.vector.vector_allowed = vector_allowed;
    }

    /// Get the current vector dirty count value
    pub(crate) fn vs_dirty_count(&self) -> u32 {
        self.vector.vs_dirty_count
    }
}

pub(crate) type TestInterpreterState<Instruction> = BasicInterpreterState<
    BasicRegisters<Reg<u64>>,
    ExtState,
    TestMemory,
    TestInstructionFetcher<Instruction>,
    TestInstructionHandler,
>;

pub(crate) fn initialize_state<I, Instructions>(
    instructions: Instructions,
) -> TestInterpreterState<I>
where
    I: Instruction<Reg = Reg<u64>>,
    Instructions: IntoIterator<Item = I>,
{
    BasicInterpreterState {
        regs: BasicRegisters::default(),
        ext_state: ExtState::default(),
        memory: TestMemory::new(8192, TEST_BASE_ADDR),
        instruction_fetcher: TestInstructionFetcher::new(
            instructions,
            TRAP_ADDRESS,
            TEST_BASE_ADDR,
            TEST_BASE_ADDR,
        ),
        system_instruction_handler: TestInstructionHandler,
    }
}

pub(crate) fn execute<I>(
    state: &mut TestInterpreterState<I>,
) -> Result<(), ExecutionError<Address<I>>>
where
    I: Instruction<Reg = Reg<u64>>
        + ExecutableInstruction<
            BasicRegisters<Reg<u64>>,
            ExtState,
            TestMemory,
            TestInstructionFetcher<I>,
            TestInstructionHandler,
        >,
{
    loop {
        let instruction = match state.instruction_fetcher.fetch_instruction(&state.memory)? {
            FetchInstructionResult::Instruction(instruction) => instruction,
            FetchInstructionResult::ControlFlow(ControlFlow::Continue(())) => {
                continue;
            }
            FetchInstructionResult::ControlFlow(ControlFlow::Break(())) => {
                break;
            }
        };

        let Rs1Rs2Operands { rs1, rs2 } = instruction.get_rs1_rs2_operands();
        let rs1rs2_values = Rs1Rs2OperandValues {
            rs1_value: state.regs.read(rs1),
            rs2_value: state.regs.read(rs2),
        };

        match instruction.execute(
            rs1rs2_values,
            &mut state.regs,
            &mut state.ext_state,
            &mut state.memory,
            &mut state.instruction_fetcher,
            &mut state.system_instruction_handler,
        )? {
            ControlFlow::Continue((rd, rd_value)) => {
                state.regs.write(rd, rd_value);
                continue;
            }
            ControlFlow::Break(()) => {
                break;
            }
        }
    }

    Ok(())
}
