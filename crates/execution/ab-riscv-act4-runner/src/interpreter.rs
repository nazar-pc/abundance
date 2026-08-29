use ab_riscv_interpreter::prelude::*;
use ab_riscv_primitives::prelude::*;
use chacha20::ChaCha8Rng;
use chacha20::rand_core::{Rng, SeedableRng};
use core::ops::ControlFlow;
use std::collections::BTreeMap;

/// Bit index of `mstatus.MIE`
const MSTATUS_MIE_BIT: u8 = 3;
/// Bit index of `mstatus.MPIE`
const MSTATUS_MPIE_BIT: u8 = 7;
/// Bit index of `mstatus.VS[0]` (2-bit field, `mstatus.VS[1]` is `MSTATUS_VS_BIT + 1`)
const MSTATUS_VS_BIT: u8 = 9;
/// Bit index of `mstatus.MPP[0]` (2-bit field, `mstatus.MPP[1]` is `MSTATUS_MPP_BIT + 1`)
const MSTATUS_MPP_BIT: u8 = 11;

/// Mask a raw `mstatus` value down to the fields this M-mode-only, F-less, S/U-less core actually
/// implements.
///
/// Per the mstatus field table: MIE and MPIE are real read/write bits; VS is a real 2-bit WARL
/// field (this core has vector support, via ZveXx); MPP is hardwired to M (`0b11`) since M is the
/// only implemented privilege mode - there is no lower mode to return to, or to hold a reservation
/// for; SD is computed from VS (FS and XS are always 0, since F and other user extensions with XS
/// state aren't implemented). Every other field (SIE/SPIE/UBE/SPP/FS/XS/MPRV/SUM/MXR/TVM/TW/TSR
/// and the WPRI ranges) is read-only zero, since S-mode and U-mode are both unimplemented.
pub(crate) fn mask_mstatus<Reg>(value: Reg::Type) -> Reg::Type
where
    Reg: Register,
{
    let one = Reg::Type::from(1u8);
    let mie = value & (one << MSTATUS_MIE_BIT);
    let mpie = value & (one << MSTATUS_MPIE_BIT);
    let vs = (value >> MSTATUS_VS_BIT) & Reg::Type::from(0b11u8);
    let mpp = Reg::Type::from(0b11u8) << MSTATUS_MPP_BIT;
    let sd = if vs == Reg::Type::from(0b11u8) {
        one << (Reg::XLEN - 1)
    } else {
        Reg::Type::default()
    };
    mie | mpie | (vs << MSTATUS_VS_BIT) | mpp | sd
}

/// Fixed `misa` value for this core.
///
/// `MISA_CSR_IMPLEMENTED: false` (per `sail.json`'s `writable_misa: false`) only means misa isn't
/// *writable* - every write is WARL-ignored - not that it reads back all zero. MXL (top 2 bits)
/// reports the hart's fixed base ISA width, and each extension-letter bit reports whether that
/// single-letter extension is actually implemented; both stay readable and accurate regardless of
/// writability (verified against the reference model). Only `I`/`M`/`A`/`B` have a corresponding
/// misa bit among this core's `implemented_extensions` - the rest are all `Z*` sub-extensions,
/// which have no misa bit of their own.
pub(crate) fn misa_value<Reg>() -> Reg::Type
where
    Reg: Register,
{
    let one = Reg::Type::from(1u8);
    let mxl = Reg::Type::from(u8::from(Reg::XLEN == 64) + 1);
    let extensions = one /* A */ | (one << 1) /* B */ | (one << 8) /* I */ | (one << 12) /* M */;
    (mxl << (Reg::XLEN - 2)) | extensions
}

/// Mask a raw `mie` value down to `MSIE`/`MTIE`/`MEIE` (bits 3, 7, 11). Every other bit is either
/// WPRI or requires S-mode/U-mode (`SSIE`/`STIE`/`SEIE`/`USIE`/`UTIE`/`UEIE`), neither of which
/// this core implements, so those are forced to read-only zero.
pub(crate) fn mask_mie<Reg>(value: Reg::Type) -> Reg::Type
where
    Reg: Register,
{
    let one = Reg::Type::from(1u8);
    value & ((one << 3) | (one << 7) | (one << 11))
}

/// Mask a raw `mcountinhibit` value: bit 1 is WPRI (reserved, always 0) - every other bit (CY,
/// IR, and the HPM3..31 inhibit bits) is plain read/write storage, since MCOUNTINHIBIT_IMPLEMENTED
/// being false only means inhibiting doesn't actually stop any counter (this core doesn't count
/// anything to begin with), not that the bits themselves aren't storable.
pub(crate) fn mask_mcountinhibit<Reg>(value: Reg::Type) -> Reg::Type
where
    Reg: Register,
{
    value & !(Reg::Type::from(1u8) << 1u8)
}

/// `mip` is fully read-only on this core, regardless of what's written to it (verified against
/// the reference model: writing all-ones then all-zeros both read back identically). `MEIP`/`SEIP`
/// would normally be wired to an external interrupt controller and `MSIP` to a CLINT-style
/// memory-mapped register - neither exists here, so both are always 0. `MTIP` is permanently 1:
/// with no real `mtimecmp` to compare against, the reference model's timer is trivially always
/// pending (mtime, starting at 0, is never less than a never-configured mtimecmp).
pub(crate) fn mask_mip<Reg>(_value: Reg::Type) -> Reg::Type
where
    Reg: Register,
{
    Reg::Type::from(1u8) << 7
}

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
        // Machine ID registers (MRO): 0 is a legal "not implemented"/"non-commercial" value for
        // all four per spec, matching MARCHID_IMPLEMENTED/MIMPID_IMPLEMENTED (both false) and
        // VENDOR_ID_BANK/VENDOR_ID_OFFSET (both 0) in the DUT config. There's only ever one hart,
        // so mhartid is 0 too.
        csrs.insert(MCsr::Mvendorid as u16, zero);
        csrs.insert(MCsr::Marchid as u16, zero);
        csrs.insert(MCsr::Mimpid as u16, zero);
        csrs.insert(MCsr::Mhartid as u16, zero);
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
        // trap can be taken. mstatus is the exception: mask_mstatus() forces MPP to M even from an
        // all-zero input, which is also the correct reset value (M is the only implemented mode).
        csrs.insert(MCsr::Mstatus as u16, mask_mstatus::<Reg>(zero));
        // MISA_CSR_IMPLEMENTED is false for this core, so misa is hardwired to 0 - correctly
        // signaling "no extensions" including no H (bit 7), which the ACT4 framework's own trap
        // handler (under STANDARD_SM_SUPPORTED) reads unconditionally on every M-mode trap to
        // decide whether to also save mtval2/mtinst
        csrs.insert(MCsr::Misa as u16, misa_value::<Reg>());
        csrs.insert(MCsr::Mie as u16, zero);
        csrs.insert(MCsr::Mtvec as u16, zero);
        csrs.insert(MCsr::Mscratch as u16, zero);
        csrs.insert(MCsr::Mepc as u16, zero);
        csrs.insert(MCsr::Mcause as u16, zero);
        csrs.insert(MCsr::Mtval as u16, zero);
        csrs.insert(MCsr::Mip as u16, mask_mip::<Reg>(zero));
        // mstatush only exists on RV32 (on RV64 its fields live in mstatus directly); the ACT4
        // framework's default M-mode boot (RVTEST_BOOT_TO_MMODE, under STANDARD_SM_SUPPORTED)
        // unconditionally clears it unless SM1P11P0_SUPPORTED is defined, which this core does
        // not define
        if Reg::XLEN == 32 {
            csrs.insert(MCsr::Mstatush as u16, zero);
            csrs.insert(MCsr::Mseccfgh as u16, zero);
        }
        // mcountinhibit must always be implemented, independent of any extension
        csrs.insert(MCsr::Mcountinhibit as u16, zero);
        // mseccfg (and mseccfgh, RV32 only, inserted above) exist because Zkr is implemented (it
        // gates SSEED/USEED access to the seed CSR), but Smepmp (MML/MMWP/RLB) isn't, and there's
        // no S/U mode for SSEED/USEED to grant access to, so every field of both halves is
        // hardwired to 0 - see instruction.rs's prepare_csr_write
        csrs.insert(MCsr::Mseccfg as u16, zero);
        // menvcfg/menvcfgh, mcycle/minstret/mcycleh/minstreth and mhpmcounter/mhpmeventN are
        // deliberately NOT inserted here: verified against the reference model (via its actual
        // execution trace, not just the embedded expected-signature values) that every access to
        // any of them traps illegal - see Sm_mcsr_access-00.sig.trace's "trapping from M to M to
        // handle illegal-instruction" entries for both csrrw and csrrs against menvcfg (0x30A).
        // sail-riscv's own source confirms why: `is_CSR_accessible` gates menvcfg/menvcfgh behind
        // `Ext_U`, mcycle/minstret/mcycleh/minstreth behind `Ext_Zicntr`, and
        // mhpmcounterN/mhpmeventN behind `Ext_Zihpm` - all three are `false` in this DUT's
        // `sail.json`. The realistic-looking values some Sm_mcsr_walk-* coverpoints appear to read
        // back are coincidental: those checkpoints reuse the same register across the (trapped,
        // no-op) CSR access and the surrounding non-CSR masking arithmetic, so the "expected" value
        // is really just that arithmetic's own result, not a real CSR read.
        //
        // mconfigptr must always be implemented (read-only); CONFIG_PTR_ADDRESS is 0 for this core
        csrs.insert(MCsr::Mconfigptr as u16, zero);
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

        // Per spec, trap entry saves the current MIE into MPIE, then clears MIE (interrupts are
        // disabled while the trap handler runs); MPP would normally record the pre-trap privilege
        // mode, but that's already forced to M unconditionally since M is the only mode this core
        // implements.
        let one = Reg::Type::from(1u8);
        let mstatus = self.csrs[&(MCsr::Mstatus as u16)];
        let mie = (mstatus >> MSTATUS_MIE_BIT) & one;
        let mstatus = (mstatus & !(one << MSTATUS_MIE_BIT) & !(one << MSTATUS_MPIE_BIT))
            | (mie << MSTATUS_MPIE_BIT);
        *self.csrs.get_mut(&(MCsr::Mstatus as u16)).unwrap() = mstatus;

        let mtvec = self.csrs[&(MCsr::Mtvec as u16)];
        if mtvec == Reg::Type::default() {
            return None;
        }

        Some(mtvec & !Reg::Type::from(0b11u8))
    }

    /// Handle `mret`: restore `mstatus.MIE` from `MPIE` (setting `MPIE` to 1 per spec), returning
    /// the target PC (`mepc`). `MPP` stays forced to M (the only implemented privilege mode).
    pub(crate) fn return_from_trap(&mut self) -> Reg::Type {
        let one = Reg::Type::from(1u8);
        let mstatus = self.csrs[&(MCsr::Mstatus as u16)];
        let mpie = (mstatus >> MSTATUS_MPIE_BIT) & one;
        let mstatus = (mstatus & !(one << MSTATUS_MIE_BIT) & !(one << MSTATUS_MPIE_BIT))
            | (mpie << MSTATUS_MIE_BIT)
            | (one << MSTATUS_MPIE_BIT);
        *self.csrs.get_mut(&(MCsr::Mstatus as u16)).unwrap() = mstatus;

        self.csrs[&(MCsr::Mepc as u16)]
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
    Self: VectorRegistersExt<Reg>,
{
    fn handle_ecall(
        &mut self,
        _regs: &mut Regs,
        memory: &mut Memory,
        program_counter: &mut PC,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type>> {
        // `ecall` always traps in M-mode (there is no less-privileged mode to delegate to, and
        // TRAP_ON_ECALL_FROM_M is true for this core) - dispatch through the trap handler like any
        // other synchronous exception, rather than treating it as an illegal instruction. Per spec,
        // `mtval` is always zero for an environment call, regardless of any DUT config parameter.
        let epc = program_counter.old_pc(size_of::<u32>() as u8);
        let trap_pc = self
            .take_trap(
                MCauseException::MachineEnvironmentCall,
                epc,
                Reg::Type::default(),
            )
            .ok_or(ExecutionError::IllegalInstruction {
                address: PackedAddress::new(epc),
            })?;
        program_counter.set_pc(memory, trap_pc)
    }

    fn handle_ebreak(
        &mut self,
        _regs: &mut Regs,
        memory: &mut Memory,
        program_counter: &mut PC,
        instruction_size: u8,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type>> {
        // `ebreak`/`c.ebreak` always trap in M-mode (TRAP_ON_EBREAK is true for this core) -
        // dispatch through the trap handler like `ecall` above, rather than the default no-op.
        // Per REPORT_VA_IN_MTVAL_ON_BREAKPOINT (false for this core), `mtval` is zero.
        let epc = program_counter.old_pc(instruction_size);
        let trap_pc = self
            .take_trap(MCauseException::Breakpoint, epc, Reg::Type::default())
            .ok_or(ExecutionError::IllegalInstruction {
                address: PackedAddress::new(epc),
            })?;
        program_counter.set_pc(memory, trap_pc)
    }
}

impl<Reg, const ELEN: Elen, const VLEN: Vlen> WrsHandler for TestEnv<Reg, ELEN, VLEN> where
    Reg: Register
{
}

impl<Reg, const ELEN: Elen, const VLEN: Vlen> FenceIHandler for TestEnv<Reg, ELEN, VLEN> where
    Reg: Register
{
}
