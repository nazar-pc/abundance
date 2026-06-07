use crate::abundance_rv64i_max::instruction::AbundanceRv64IMaxInstruction;
use ab_riscv_interpreter::prelude::*;
use ab_riscv_primitives::prelude::*;
use std::collections::BTreeMap;

const ELEN: u32 = u64::BITS;
const VLEN: u32 = 1024;

pub(crate) struct AbundanceRv64IMaxExtState {
    csrs: BTreeMap<u16, u64>,
    vregs: VectorRegisterFile<{ Self::VLENB as usize }>,
    vtype: Option<Vtype<{ Self::ELEN }, { Self::VLEN }>>,
    vtype_raw: u64,
    vl: u32,
}

impl AbundanceRv64IMaxExtState {
    pub(crate) fn new() -> Self {
        let mut s = Self {
            csrs: BTreeMap::new(),
            vregs: VectorRegisterFile::default(),
            vtype: None,
            vtype_raw: 1u64 << (u64::BITS - 1),
            vl: 0,
        };
        // Vector CSRs
        s.init_csr(VCsr::Vstart as u16, 0);
        s.init_csr(VCsr::Vxsat as u16, 0);
        s.init_csr(VCsr::Vxrm as u16, 0);
        s.init_csr(VCsr::Vcsr as u16, 0);
        s.init_csr(VCsr::Vl as u16, 0);
        s.init_csr(VCsr::Vtype as u16, 1u64 << (u64::BITS - 1));
        s.init_csr(VCsr::Vlenb as u16, u64::from(Self::VLEN / u8::BITS));
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

    fn init_csr(&mut self, idx: u16, val: u64) {
        self.csrs.insert(idx, val);
    }

    /// Dispatch a synchronous trap, returning the new PC (mtvec target).
    ///
    /// Writes `mepc`, `mcause`, `mtval`, then returns `mtvec & !0b11` (direct mode only -
    /// MTVEC_MODES: `[0]`). If mtvec is zero, the test never set it up, which means the test
    /// doesn't expect traps; return None so the caller can treat it as a hard error.
    pub(crate) fn take_trap<Cause>(&mut self, cause: Cause, epc: u64, tval: u64) -> Option<u64>
    where
        MCause: From<Cause>,
    {
        *self.csrs.get_mut(&(MCsr::Mepc as u16)).unwrap() = epc;
        *self.csrs.get_mut(&(MCsr::Mcause as u16)).unwrap() =
            MCause::from(cause).to_raw::<Reg<u64>>();
        *self.csrs.get_mut(&(MCsr::Mtval as u16)).unwrap() = tval;

        let mtvec = *self.csrs.get(&(MCsr::Mtvec as u16)).unwrap();
        if mtvec == 0 {
            return None;
        }

        Some(mtvec & !0b11)
    }
}

impl Csrs<<AbundanceRv64IMaxInstruction as Instruction>::Reg> for AbundanceRv64IMaxExtState {
    fn privilege_level(&self) -> PrivilegeLevel {
        PrivilegeLevel::Machine
    }

    fn read_csr(&self, csr_index: u16) -> Result<u64, CsrError> {
        self.csrs
            .get(&csr_index)
            .copied()
            .ok_or(CsrError::IllegalRead { csr_index })
    }

    fn write_csr(&mut self, csr_index: u16, value: u64) -> Result<(), CsrError> {
        let slot = self
            .csrs
            .get_mut(&csr_index)
            .ok_or(CsrError::IllegalWrite { csr_index })?;
        *slot = value;
        Ok(())
    }
}

impl VectorRegistersBase for AbundanceRv64IMaxExtState {
    const ELEN: u32 = ELEN;
    const VLEN: u32 = VLEN;
}

impl VectorRegisters for AbundanceRv64IMaxExtState
where
    Self: Csrs<<AbundanceRv64IMaxInstruction as Instruction>::Reg>,
{
    fn read_vregs(&self) -> &VectorRegisterFile<{ Self::VLENB as usize }> {
        &self.vregs
    }
    fn write_vregs(&mut self) -> &mut VectorRegisterFile<{ Self::VLENB as usize }> {
        &mut self.vregs
    }
    fn vtype(&self) -> Option<Vtype<{ Self::ELEN }, { Self::VLEN }>> {
        self.vtype
    }
    fn set_vtype(&mut self, vtype: Option<Vtype<{ Self::ELEN }, { Self::VLEN }>>) {
        self.vtype = vtype;
        let raw = match vtype {
            Some(vt) => vt.to_raw::<<AbundanceRv64IMaxInstruction as Instruction>::Reg>(),
            None => 1u64 << (u64::BITS - 1),
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
        self.write_csr(VCsr::Vl as u16, u64::from(vl))
            .expect("vl CSR not initialized");
    }
    fn vector_instructions_allowed(&self) -> bool {
        true
    }
    fn mark_vs_dirty(&mut self) {}
}

impl VectorRegistersExt<<AbundanceRv64IMaxInstruction as Instruction>::Reg>
    for AbundanceRv64IMaxExtState
{
}
