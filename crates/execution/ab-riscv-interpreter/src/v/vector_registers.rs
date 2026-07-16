//! Vector registers

use crate::v::private::SupportedElenVlen;
use crate::{Csrs, CustomErrorPlaceholder};
use ab_riscv_primitives::prelude::*;
use core::fmt;

const VLENB<const VLEN: u32>: u32 = VLEN / u8::BITS;
pub(crate) const VLENB_USIZE<const VLENB: u32>: usize = VLENB as usize;

/// Alignment wrapper for vector registers
#[derive(Debug, Clone, Copy)]
// Aligned to 128 bytes, which is u32 * 32 registers, the minimum reasonable value to use in most
// cases
#[repr(align(128))]
pub struct VectorRegisterFile<const VLENB: u32>([[u8; VLENB_USIZE::<VLENB>]; 32]);

const impl<const VLENB: u32> Default for VectorRegisterFile<VLENB> {
    #[inline(always)]
    fn default() -> Self {
        Self([[0; _]; _])
    }
}

impl<const VLENB: u32> VectorRegisterFile<VLENB> {
    /// Get reference to a vector register
    #[inline(always)]
    pub fn get(&self, index: VReg) -> &[u8; VLENB_USIZE::<VLENB>] {
        // SAFETY: Always in-range
        unsafe { self.0.get_unchecked(usize::from(index.to_bits())) }
    }

    /// Get mutable reference to a vector register
    #[inline(always)]
    pub fn get_mut(&mut self, index: VReg) -> &mut [u8; VLENB_USIZE::<VLENB>] {
        // SAFETY: Always in-range
        unsafe { self.0.get_unchecked_mut(usize::from(index.to_bits())) }
    }
}

/// Base for [`VectorRegisters`].
///
/// This is primarily a workaround for type system cycles.
pub trait VectorRegistersBase {
    /// Maximum vector element width `ELEN` in bits
    const ELEN: u32;
    /// Vector register width `VLEN` in bits
    const VLEN: u32;
    /// Vector register width in bytes (`vlenb = VLEN / 8`)
    const VLENB: u32 = VLENB::<{ Self::VLEN }>;
    /// Vector register width in bytes (`vlenb = VLEN / 8`).
    ///
    /// The same as `Self::VLENB`, but `usize`.
    const VLENB_USIZE: usize = VLENB_USIZE::<{ Self::VLENB }>;
}

// TODO: Figure out a way to make `VectorRegisters + VectorRegistersExt` trait bounds work without
//  type system cycles
/// Vector register state.
///
/// This trait contains only methods that implementations genuinely need to provide. Derived
/// accessors for simpler CSRs are in [`VectorRegistersExt`].
///
/// Note that due to Rust type system limitations, you should use [`VectorRegistersExt`] in trait
/// bounds instead of this trait directly or else the solver will fail.
///
/// Methods for `vtype` and `vl` live here (not in the ext trait) because they have non-trivial
/// update semantics: `vtype` must maintain a cached decoded form and handle the XLEN-dependent vill
/// bit, and `vl` is read-only via CSR instructions but writable by `vsetvl{i}` and fault-only-first
/// loads.
///
/// `ELEN` is the maximum element width in bits.
pub trait VectorRegisters<CustomError = CustomErrorPlaceholder>
where
    Self: VectorRegistersBase + SupportedElenVlen<{ Self::ELEN }, { Self::VLEN }>,
{
    /// Read the vector register file
    fn read_vregs(&self) -> &VectorRegisterFile<{ Self::VLENB }>;

    /// Mutable access to the vector register file
    fn write_vregs(&mut self) -> &mut VectorRegisterFile<{ Self::VLENB }>;

    /// Check whether vector instructions are currently permitted.
    ///
    /// Returns `false` when `mstatus.VS == Off` (or equivalent like `sstatus`/`vstatus`). In
    /// environments without these status registers, returns `true` always.
    fn vector_instructions_allowed(&self) -> bool;

    /// Mark the vector state as dirty.
    ///
    /// Must set VS to Dirty in `mstatus` (and `sstatus`/`vsstatus` shadows) when those registers
    /// exist. No-op otherwise.
    fn mark_vs_dirty(&mut self);

    /// Compute `vl` from `AVL` and `VLMAX` per spec constraints.
    ///
    /// The simplest compliant implementation (which is used by default) is `min(AVL, VLMAX)`. More
    /// sophisticated implementations may return values in `[ceil(AVL/2), VLMAX]` for
    /// `AVL < 2*VLMAX`, but this simple strategy satisfies all three spec requirements.
    #[inline(always)]
    fn compute_vl(&self, avl: u32, vlmax: u32) -> u32 {
        avl.min(vlmax)
    }

    /// Compute `VLMAX` for a given vtype
    #[inline(always)]
    fn vlmax_for_vtype(&self, vtype: Vtype<{ Self::ELEN }, { Self::VLEN }>) -> u32 {
        vtype.vlmul().vlmax::<{ Self::VLEN }>(vtype.vsew())
    }
}

/// Derived convenience accessors for vector CSRs that are simple read/write fields (vstart, vxrm,
/// vxsat, vcsr).
///
/// Intended for types that implement both [`VectorRegisters`] and [`Csrs`].
///
/// NOTE: While the default methods implemented via the [`Csrs`] trait are correct, custom
/// higher-performance implementations are often possible by overriding them and, for example,
/// caching various CSRs as separate pre-decoded values rather than going through a generic code
/// path with XLEN-sized raw CSR values during reads.
pub trait VectorRegistersExt<Reg, CustomError = CustomErrorPlaceholder>
where
    Self: Csrs<Reg, CustomError> + VectorRegisters<CustomError>,
    Reg: Register,
    CustomError: fmt::Debug,
{
    /// Initialize the vector state to the recommended default configuration.
    ///
    /// Per spec: `vtype.vill` = 1, remaining `vtype` bits = `0`, `vl` = 0.
    /// `vstart`, `vxrm`, `vxsat` may have arbitrary values at reset but are zeroed here for
    /// deterministic behavior.
    fn initialize_vector_state(&mut self) {
        self.set_vtype(None);
        self.set_vl(0);
        self.set_vstart(0);
        self.set_vxrm(Vxrm::default());
        self.set_vxsat(false);
    }

    /// Get current `vstart`
    #[inline(always)]
    fn vstart(&self) -> u16 {
        let raw = self
            .read_csr(VectorCsr::Vstart.to_csr_index())
            .unwrap_or_default()
            .as_u64();
        raw as u16
    }

    /// Set `vstart`
    #[inline(always)]
    fn set_vstart(&mut self, vstart: u16) {
        self.write_csr(VectorCsr::Vstart.to_csr_index(), Reg::Type::from(vstart))
            .expect("Implementation didn't initialize `vstart` CSR");
    }

    /// Reset `vstart` to zero.
    ///
    /// Per spec, all vector instructions reset `vstart` to zero at the end of execution.
    #[inline(always)]
    fn reset_vstart(&mut self) {
        self.set_vstart(0);
    }

    /// Get `vxsat` (single bit)
    #[inline(always)]
    fn vxsat(&self) -> bool {
        let raw = self
            .read_csr(VectorCsr::Vxsat.to_csr_index())
            .unwrap_or_default()
            .as_u64();
        (raw & 1) == 1
    }

    /// Set `vxsat`
    #[inline(always)]
    fn set_vxsat(&mut self, vxsat: bool) {
        let masked = Reg::Type::from(u8::from(vxsat));
        self.write_csr(VectorCsr::Vxsat.to_csr_index(), masked)
            .expect("Implementation didn't initialize `vxsat` CSR");
        // Mirror `vxsat` into `vcsr[0]`, preserving `vcsr[2:1]` (`vxrm`)
        let old_vcsr = self
            .read_csr(VectorCsr::Vcsr.to_csr_index())
            .unwrap_or_default();
        let new_vcsr = (old_vcsr & !Reg::Type::from(1u8)) | masked;
        self.write_csr(VectorCsr::Vcsr.to_csr_index(), new_vcsr)
            .expect("Implementation didn't initialize `vcsr` CSR");
    }

    /// Get `vxrm`
    #[inline(always)]
    fn vxrm(&self) -> Vxrm {
        let raw = self
            .read_csr(VectorCsr::Vxrm.to_csr_index())
            .unwrap_or_default()
            .as_u64();
        Vxrm::from_bits(raw as u8)
    }

    /// Set `vxrm`
    #[inline(always)]
    fn set_vxrm(&mut self, vxrm: Vxrm) {
        let masked = Reg::Type::from(vxrm.to_bits());
        self.write_csr(VectorCsr::Vxrm.to_csr_index(), masked)
            .expect("Implementation didn't initialize `vxrm` CSR");
        // Mirror `vxrm` into `vcsr[2:1]`, preserving `vcsr[0]` (`vxsat`)
        let old_vcsr = self
            .read_csr(VectorCsr::Vcsr.to_csr_index())
            .unwrap_or_default();
        let new_vcsr = (old_vcsr & !Reg::Type::from(0b110u8)) | (masked << 1u8);
        self.write_csr(VectorCsr::Vcsr.to_csr_index(), new_vcsr)
            .expect("Implementation didn't initialize `vcsr` CSR");
    }

    // TODO: Consider new type for `vl`. It is guaranteed to be `at most u16::MAX + 1`, so can be a
    //  wrapper around `u16`, which will make things nicer in some places like when iterating over
    //  `vstart..vl`
    /// Get the current vl
    #[inline(always)]
    fn vl(&self) -> u32 {
        self.read_csr(VectorCsr::Vl.to_csr_index())
            .unwrap_or_default()
            .as_u64() as u32
    }

    /// Set vl.
    ///
    /// The implementation must update both its internal decoded cache and the raw CSR value (for
    /// reads via Zicsr, writes via Zicsr are not allowed).
    fn set_vl(&mut self, vl: u32) {
        self.write_csr(VectorCsr::Vl.to_csr_index(), Reg::Type::from(vl))
            .expect("Implementation didn't initialize `vl` CSR");
    }

    /// Get the current decoded vtype
    #[inline(always)]
    fn vtype(&self) -> Option<Vtype<{ Self::ELEN }, { Self::VLEN }>> {
        self.read_csr(VectorCsr::Vtype.to_csr_index())
            .ok()
            .and_then(Vtype::from_raw::<Reg>)
    }

    /// Set the vtype register from a decoded `Vtype`.
    ///
    /// The implementation must update both its internal decoded cache and the raw CSR value (for
    /// reads via Zicsr, writes via Zicsr are not allowed).
    #[inline(always)]
    fn set_vtype(&mut self, vtype: Option<Vtype<{ Self::ELEN }, { Self::VLEN }>>) {
        let vtype_raw = if let Some(vt) = vtype {
            vt.to_raw::<Reg>()
        } else {
            Vtype::<{ Self::ELEN }, { Self::VLEN }>::illegal_raw::<Reg>()
        };

        self.write_csr(VectorCsr::Vtype.to_csr_index(), vtype_raw)
            .expect("Implementation didn't initialize `vtype` CSR");
    }
}
