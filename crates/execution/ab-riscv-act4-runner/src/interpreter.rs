use crate::instruction::{MHPMEVENT3_CSR_INDEX, MHPMEVENT31_CSR_INDEX};
use ab_riscv_interpreter::prelude::*;
use ab_riscv_primitives::prelude::*;
use chacha20::ChaCha8Rng;
use chacha20::rand_core::{Rng, SeedableRng};
use core::ops::ControlFlow;
use std::collections::BTreeMap;

/// Execution environment of the core under test
pub(crate) struct TestEnv<Reg, const ELEN: Elen, const VLEN: Vlen>
where
    Reg: Register,
{
    csrs: BTreeMap<u16, Reg::Type>,
    vregs: VectorRegisterFile<VLEN>,
    reservation: Option<Reg::Type>,
    entropy_source: ChaCha8Rng,
}

impl<Reg, const ELEN: Elen, const VLEN: Vlen> TestEnv<Reg, ELEN, VLEN>
where
    Reg: Register,
    Self: VectorRegistersExt<Reg>,
{
    /// Create a new instance with all CSRs the tests expect to exist initialized
    pub(crate) fn new() -> Self {
        let zero = Reg::Type::default();

        let mut csrs = BTreeMap::new();
        // Vector CSRs
        csrs.insert(VectorCsr::Vstart.to_csr_index(), zero);
        csrs.insert(VectorCsr::Vxsat.to_csr_index(), zero);
        csrs.insert(VectorCsr::Vxrm.to_csr_index(), zero);
        csrs.insert(VectorCsr::Vcsr.to_csr_index(), zero);
        csrs.insert(VectorCsr::Vl.to_csr_index(), zero);
        csrs.insert(
            VectorCsr::Vtype.to_csr_index(),
            Reg::Type::from(1u8) << (Reg::XLEN - 1),
        );
        csrs.insert(
            VectorCsr::Vlenb.to_csr_index(),
            Reg::Type::from(VLEN.bytes()),
        );
        // Machine trap CSRs - zero-initialized, mtvec must be written by test boot code before any
        // trap can be taken
        csrs.insert(MCsr::Mstatus as u16, zero);
        // MISA_CSR_IMPLEMENTED is false for this core, so misa is hardwired to 0 - correctly
        // signaling "no extensions" including no H (bit 7), which the ACT4 framework's own trap
        // handler (under STANDARD_SM_SUPPORTED) reads unconditionally on every M-mode trap to
        // decide whether to also save mtval2/mtinst
        csrs.insert(MCsr::Misa as u16, zero);
        csrs.insert(MCsr::Mie as u16, zero);
        csrs.insert(MCsr::Mtvec as u16, zero);
        csrs.insert(MCsr::Mscratch as u16, zero);
        csrs.insert(MCsr::Mepc as u16, zero);
        csrs.insert(MCsr::Mcause as u16, zero);
        csrs.insert(MCsr::Mtval as u16, zero);
        csrs.insert(MCsr::Mip as u16, zero);
        // mstatush only exists on RV32 (on RV64 its fields live in mstatus directly); the ACT4
        // framework's default M-mode boot (RVTEST_BOOT_TO_MMODE, under STANDARD_SM_SUPPORTED)
        // unconditionally clears it unless SM1P11P0_SUPPORTED is defined, which this core does
        // not define
        if Reg::XLEN == 32 {
            csrs.insert(MCsr::Mstatush as u16, zero);
        }
        // mcountinhibit and mhpmevent3..31 are unconditionally zeroed by the same boot sequence
        // ("They must be implemented" per its own comment) regardless of whether this core actually
        // implements extra hardware performance counters
        csrs.insert(MCsr::Mcountinhibit as u16, zero);
        for csr_index in MHPMEVENT3_CSR_INDEX..=MHPMEVENT31_CSR_INDEX {
            csrs.insert(csr_index, zero);
        }
        csrs.insert(SEED_CSR_INDEX, zero);

        let mut s = Self {
            csrs,
            vregs: VectorRegisterFile::default(),
            reservation: None,
            // Good enough for testing purposes
            entropy_source: ChaCha8Rng::from_seed([0; _]),
        };
        s.initialize_vector_state();
        s
    }

    /// Dispatch a synchronous trap, returning the new PC (mtvec target).
    ///
    /// Writes `mepc`, `mcause`, `mtval`, then returns `mtvec & !0b11` (direct mode only -
    /// MTVEC_MODES: `[0]`). If mtvec is zero, the test never set it up, which means the test
    /// doesn't expect traps; return None so the caller can treat it as a hard error.
    pub(crate) fn take_trap<Cause>(
        &mut self,
        cause: Cause,
        epc: Reg::Type,
        tval: Reg::Type,
    ) -> Option<Reg::Type>
    where
        MCause: From<Cause>,
    {
        *self.csrs.get_mut(&(MCsr::Mepc as u16)).unwrap() = epc;
        *self.csrs.get_mut(&(MCsr::Mcause as u16)).unwrap() = MCause::from(cause).to_raw::<Reg>();
        *self.csrs.get_mut(&(MCsr::Mtval as u16)).unwrap() = tval;

        let mtvec = self.csrs[&(MCsr::Mtvec as u16)];
        if mtvec == Reg::Type::default() {
            return None;
        }

        Some(mtvec & !Reg::Type::from(0b11u8))
    }
}

impl<Reg, const ELEN: Elen, const VLEN: Vlen> Csrs<Reg> for TestEnv<Reg, ELEN, VLEN>
where
    Reg: Register,
{
    fn privilege_level(&self) -> PrivilegeLevel {
        PrivilegeLevel::Machine
    }

    fn read_csr(&self, csr_index: u16) -> Result<Reg::Type, CsrError> {
        self.csrs
            .get(&csr_index)
            .copied()
            .ok_or(CsrError::IllegalRead { csr_index })
    }

    fn write_csr(&mut self, csr_index: u16, value: Reg::Type) -> Result<(), CsrError> {
        let slot = self
            .csrs
            .get_mut(&csr_index)
            .ok_or(CsrError::IllegalWrite { csr_index })?;
        *slot = value;
        Ok(())
    }
}

// TODO: The compiler does not normalize `<Self as VectorRegisters>::VLEN` (as used in the
//  signatures of the methods below) to the `VLEN` const generic while `Self` is generic, so this
//  impl has to be instantiated for concrete parameters instead of being generic like the rest of
//  them: https://github.com/rust-lang/rust/issues/161264
macro_rules! impl_vector_registers {
    ($reg:ty, $elen:expr, $vlen:expr) => {
        impl VectorRegisters for TestEnv<$reg, { $elen }, { $vlen }> {
            const ELEN: Elen = $elen;
            const VLEN: Vlen = $vlen;

            fn read_vregs(&self) -> &VectorRegisterFile<{ Self::VLEN }> {
                &self.vregs
            }

            fn write_vregs(&mut self) -> &mut VectorRegisterFile<{ Self::VLEN }> {
                &mut self.vregs
            }

            fn vector_instructions_allowed(&self) -> bool {
                true
            }

            fn mark_vs_dirty(&mut self) {}
        }
    };
}

impl_vector_registers!(Reg<u32>, Elen::L64, Vlen::L1024);
impl_vector_registers!(Reg<u64>, Elen::L64, Vlen::L1024);

impl<Reg, const ELEN: Elen, const VLEN: Vlen> VectorRegistersExt<Reg> for TestEnv<Reg, ELEN, VLEN>
where
    Reg: Register,
    Self: VectorRegisters,
{
}

impl<Reg, const ELEN: Elen, const VLEN: Vlen> ReservationSet<Reg> for TestEnv<Reg, ELEN, VLEN>
where
    Reg: Register,
{
    fn reservation(&self) -> Option<Reg::Type> {
        self.reservation
    }

    fn set_reservation(&mut self, address: Reg::Type) {
        self.reservation = Some(address);
    }

    fn clear_reservation(&mut self) {
        self.reservation = None;
    }
}

impl<Reg, const ELEN: Elen, const VLEN: Vlen> ZkrSeedSource for TestEnv<Reg, ELEN, VLEN>
where
    Reg: Register,
{
    fn poll_seed(&mut self) -> ZkrSeedPoll {
        let mut randomness = [0; _];
        self.entropy_source.fill_bytes(&mut randomness);
        ZkrSeedPoll::Es16(u16::from_le_bytes(randomness))
    }
}

impl<Reg, Regs, Memory, PC, const ELEN: Elen, const VLEN: Vlen>
    SystemInstructionHandler<Reg, Regs, Memory, PC> for TestEnv<Reg, ELEN, VLEN>
where
    Reg: Register,
    PC: ProgramCounter<Reg::Type, Memory>,
{
    fn handle_ecall(
        &mut self,
        _regs: &mut Regs,
        _memory: &mut Memory,
        program_counter: &mut PC,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type>> {
        Err(ExecutionError::IllegalInstruction {
            address: PackedAddress::new(program_counter.old_pc(size_of::<u32>() as u8)),
        })
    }
}

impl<Reg, const ELEN: Elen, const VLEN: Vlen> WrsHandler for TestEnv<Reg, ELEN, VLEN> where
    Reg: Register
{
}
