use crate::abundance_rv32i_max::instruction::AbundanceRv32IMaxInstruction;
use crate::interpreter::{Act4InstructionFetcher, Act4Memory, Act4SystemHandler};
use ab_riscv_interpreter::v::vector_registers::{
    VectorRegisterFile, VectorRegisters, VectorRegistersBase, VectorRegistersExt,
};
use ab_riscv_interpreter::{CsrError, Csrs, ExecutableInstruction, InterpreterState};
use ab_riscv_primitives::prelude::*;
use std::collections::BTreeMap;

const ELEN: u32 = u64::BITS;
const VLEN: u32 = 1024;

pub(crate) struct AbundanceRv32IMaxExtState {
    csrs: BTreeMap<u16, u32>,
    vregs: VectorRegisterFile<{ Self::VLENB as usize }>,
    vtype: Option<Vtype<{ Self::ELEN }, { Self::VLEN }>>,
    vtype_raw: u32,
    vl: u32,
}

impl AbundanceRv32IMaxExtState {
    pub(crate) fn new() -> Self {
        let mut s = Self {
            csrs: BTreeMap::new(),
            vregs: VectorRegisterFile::default(),
            vtype: None,
            vtype_raw: 1u32 << (u32::BITS - 1),
            vl: 0,
        };
        // Vector CSRs
        s.init_csr(VCsr::Vstart as u16, 0);
        s.init_csr(VCsr::Vxsat as u16, 0);
        s.init_csr(VCsr::Vxrm as u16, 0);
        s.init_csr(VCsr::Vcsr as u16, 0);
        s.init_csr(VCsr::Vl as u16, 0);
        s.init_csr(VCsr::Vtype as u16, 1u32 << (u32::BITS - 1));
        s.init_csr(VCsr::Vlenb as u16, Self::VLEN / u8::BITS);
        // Machine trap CSRs - zero-initialized, mtvec must be written by test
        // boot code before any trap can be taken.
        s.init_csr(MCsr::Mstatus as u16, 0);
        s.init_csr(MCsr::Mtvec as u16, 0);
        s.init_csr(MCsr::Mscratch as u16, 0);
        s.init_csr(MCsr::Mepc as u16, 0);
        s.init_csr(MCsr::Mcause as u16, 0);
        s.init_csr(MCsr::Mtval as u16, 0);
        s.initialize_vector_state();
        s
    }

    fn init_csr(&mut self, idx: u16, val: u32) {
        self.csrs.insert(idx, val);
    }

    /// Dispatch a synchronous trap, returning the new PC (mtvec target).
    ///
    /// Writes `mepc`, `mcause`, `mtval`, then returns `mtvec & !0b11` (direct mode only -
    /// MTVEC_MODES: `[0]`). If mtvec is zero, the test never set it up, which means the test
    /// doesn't expect traps; return None so the caller can treat it as a hard error.
    pub(crate) fn take_trap<Cause>(&mut self, cause: Cause, epc: u32, tval: u32) -> Option<u32>
    where
        MCause: From<Cause>,
    {
        *self.csrs.get_mut(&(MCsr::Mepc as u16)).unwrap() = epc;
        *self.csrs.get_mut(&(MCsr::Mcause as u16)).unwrap() =
            MCause::from(cause).to_raw::<Reg<u32>>();
        *self.csrs.get_mut(&(MCsr::Mtval as u16)).unwrap() = tval;

        let mtvec = *self.csrs.get(&(MCsr::Mtvec as u16)).unwrap();
        if mtvec == 0 {
            return None;
        }

        Some(mtvec & !0b11)
    }
}

impl Csrs<<AbundanceRv32IMaxInstruction as Instruction>::Reg> for AbundanceRv32IMaxExtState {
    fn privilege_level(&self) -> PrivilegeLevel {
        PrivilegeLevel::Machine
    }

    fn read_csr(&self, csr_index: u16) -> Result<u32, CsrError> {
        self.csrs
            .get(&csr_index)
            .copied()
            .ok_or(CsrError::IllegalRead { csr_index })
    }

    fn write_csr(&mut self, csr_index: u16, value: u32) -> Result<(), CsrError> {
        let slot = self
            .csrs
            .get_mut(&csr_index)
            .ok_or(CsrError::IllegalWrite { csr_index })?;
        *slot = value;
        Ok(())
    }

    fn process_csr_read(&self, csr_index: u16, raw_value: u32) -> Result<u32, CsrError> {
        let mut out = 0;
        if !<AbundanceRv32IMaxInstruction as ExecutableInstruction<
            InterpreterState<
                <AbundanceRv32IMaxInstruction as Instruction>::Reg,
                Self,
                Act4Memory<0, 0>,
                Act4InstructionFetcher<AbundanceRv32IMaxInstruction>,
                Act4SystemHandler,
                _,
            >,
            _,
        >>::prepare_csr_read(self, csr_index, raw_value, &mut out)?
        {
            return Err(CsrError::IllegalRead { csr_index });
        }

        Ok(out)
    }

    fn process_csr_write(&mut self, csr_index: u16, write_value: u32) -> Result<u32, CsrError> {
        let mut out = 0;
        if !<AbundanceRv32IMaxInstruction as ExecutableInstruction<
            InterpreterState<
                <AbundanceRv32IMaxInstruction as Instruction>::Reg,
                Self,
                Act4Memory<0, 0>,
                Act4InstructionFetcher<AbundanceRv32IMaxInstruction>,
                Act4SystemHandler,
                _,
            >,
            _,
        >>::prepare_csr_write(self, csr_index, write_value, &mut out)?
        {
            return Err(CsrError::IllegalWrite { csr_index });
        }

        Ok(out)
    }
}

impl VectorRegistersBase for AbundanceRv32IMaxExtState {
    const ELEN: u32 = ELEN;
    const VLEN: u32 = VLEN;
}

impl VectorRegisters for AbundanceRv32IMaxExtState
where
    Self: Csrs<<AbundanceRv32IMaxInstruction as Instruction>::Reg>,
{
    fn read_vreg(&self) -> &VectorRegisterFile<{ Self::VLENB as usize }> {
        &self.vregs
    }
    fn write_vreg(&mut self) -> &mut VectorRegisterFile<{ Self::VLENB as usize }> {
        &mut self.vregs
    }
    fn vtype(&self) -> Option<Vtype<{ Self::ELEN }, { Self::VLEN }>> {
        self.vtype
    }
    fn set_vtype(&mut self, vtype: Option<Vtype<{ Self::ELEN }, { Self::VLEN }>>) {
        self.vtype = vtype;
        let raw = match vtype {
            Some(vt) => vt.to_raw::<<AbundanceRv32IMaxInstruction as Instruction>::Reg>(),
            None => 1u32 << (u32::BITS - 1),
        };
        self.vtype_raw = raw;
        self.write_csr(VCsr::Vtype as u16, raw)
            .expect("vtype CSR not initialized");
    }
    fn vl(&self) -> u32 {
        self.vl
    }
    fn set_vl(&mut self, vl: u32) {
        self.vl = vl;
        self.write_csr(VCsr::Vl as u16, vl)
            .expect("vl CSR not initialized");
    }
    fn vector_instructions_allowed(&self) -> bool {
        true
    }
    fn mark_vs_dirty(&mut self) {}
}

impl VectorRegistersExt<<AbundanceRv32IMaxInstruction as Instruction>::Reg>
    for AbundanceRv32IMaxExtState
{
}
