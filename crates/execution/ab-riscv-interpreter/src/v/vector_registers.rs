//! Vector registers

use crate::Csrs;
use ab_riscv_primitives::prelude::*;

pub(crate) const VLENB_USIZE<const VLEN: Vlen>: usize = VLEN.bytes() as usize;

/// Alignment wrapper for vector registers
#[derive(Debug, Clone, Copy)]
// Aligned to 128 bytes, which is u32 * 32 registers, the minimum reasonable value to use in most
// cases
#[repr(align(128))]
pub struct VectorRegisterFile<const VLEN: Vlen>([[u8; VLENB_USIZE::<VLEN>]; 32]);

const impl<const VLEN: Vlen> Default for VectorRegisterFile<VLEN> {
    #[inline(always)]
    fn default() -> Self {
        Self([[0; _]; _])
    }
}

impl<const VLEN: Vlen> VectorRegisterFile<VLEN> {
    /// Get reference to a vector register
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    pub const fn get(&self, index: VReg) -> &[u8; VLENB_USIZE::<VLEN>] {
        // SAFETY: Always in-range
        unsafe { self.0.get_unchecked(usize::from(index.to_bits())) }
    }

    /// Get mutable reference to a vector register
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    pub const fn get_mut(&mut self, index: VReg) -> &mut [u8; VLENB_USIZE::<VLEN>] {
        // SAFETY: Always in-range
        unsafe { self.0.get_unchecked_mut(usize::from(index.to_bits())) }
    }
}

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
pub const trait VectorRegisters {
    /// Maximum vector element width `ELEN` in bits
    const ELEN: Elen;
    /// Vector register width `VLEN` in bits
    const VLEN: Vlen;

    /// Read the vector register file
    fn read_vregs(&self) -> &VectorRegisterFile<{ Self::VLEN }>;

    /// Mutable access to the vector register file
    fn write_vregs(&mut self) -> &mut VectorRegisterFile<{ Self::VLEN }>;

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
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn compute_vl(&self, avl: Vl, vlmax: Vl) -> Vl {
        avl.min(vlmax)
    }

    /// Compute `VLMAX` for a given vtype
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn vlmax_for_vtype(&self, vtype: Vtype<{ Self::ELEN }, { Self::VLEN }>) -> Vl
    where
        [(); SUPPORTED_ELEN_VLEN::<{ Self::ELEN }, { Self::VLEN }>]:,
    {
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
pub const trait VectorRegistersExt<Reg>
where
    Self: [const] Csrs<Reg> + [const] VectorRegisters,
    [(); SUPPORTED_ELEN_VLEN::<{ Self::ELEN }, { Self::VLEN }>]:,
    Reg: [const] Register,
{
    /// Initialize the vector state to the recommended default configuration.
    ///
    /// Per spec: `vtype.vill` = 1, remaining `vtype` bits = `0`, `vl` = 0.
    /// `vstart`, `vxrm`, `vxsat` may have arbitrary values at reset but are zeroed here for
    /// deterministic behavior.
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn initialize_vector_state(&mut self) {
        self.set_vtype(None);
        self.set_vl(Vl::ZERO);
        self.set_vstart(Vstart::ZERO);
        self.set_vxrm(Vxrm::default());
        self.set_vxsat(false);
    }

    /// Get current `vstart`
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn vstart(&self) -> Vstart {
        let raw = self
            .read_csr(VectorCsr::Vstart.to_csr_index())
            .unwrap_or_default()
            .as_u64();
        Vstart::from(raw as u16)
    }

    /// Set `vstart`.
    ///
    /// The default implementation ignores writes to uninitialized CSR in release mode and panics in
    /// debug.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn set_vstart(&mut self, vstart: Vstart) {
        let result = self.write_csr(
            VectorCsr::Vstart.to_csr_index(),
            Reg::Type::from(u16::from(vstart)),
        );
        debug_assert!(
            result.is_ok(),
            "Implementation must initialize `vstart` CSR"
        );
    }

    /// Reset `vstart` to zero.
    ///
    /// Per spec, all vector instructions reset `vstart` to zero at the end of execution.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn reset_vstart(&mut self) {
        self.set_vstart(Vstart::ZERO);
    }

    /// Get `vxsat` (single bit)
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn vxsat(&self) -> bool {
        let raw = self
            .read_csr(VectorCsr::Vxsat.to_csr_index())
            .unwrap_or_default()
            .as_u64();
        (raw & 1) == 1
    }

    /// Set `vxsat`.
    ///
    /// The default implementation ignores writes to uninitialized CSR in release mode and panics in
    /// debug.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn set_vxsat(&mut self, vxsat: bool) {
        let masked = Reg::Type::from(u8::from(vxsat));
        let result = self.write_csr(VectorCsr::Vxsat.to_csr_index(), masked);
        debug_assert!(result.is_ok(), "Implementation must initialize `vxsat` CSR");
        // Mirror `vxsat` into `vcsr[0]`, preserving `vcsr[2:1]` (`vxrm`)
        let old_vcsr = self
            .read_csr(VectorCsr::Vcsr.to_csr_index())
            .unwrap_or_default();
        let new_vcsr = (old_vcsr & !Reg::Type::from(1u8)) | masked;
        let result = self.write_csr(VectorCsr::Vcsr.to_csr_index(), new_vcsr);
        debug_assert!(result.is_ok(), "Implementation must initialize `vcsr` CSR");
    }

    /// Get `vxrm`
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn vxrm(&self) -> Vxrm {
        let raw = self
            .read_csr(VectorCsr::Vxrm.to_csr_index())
            .unwrap_or_default()
            .as_u64();
        Vxrm::from_bits(raw as u8)
    }

    /// Set `vxrm`.
    ///
    /// The default implementation ignores writes to uninitialized CSR in release mode and panics in
    /// debug.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn set_vxrm(&mut self, vxrm: Vxrm) {
        let masked = Reg::Type::from(vxrm.to_bits());
        let result = self.write_csr(VectorCsr::Vxrm.to_csr_index(), masked);
        debug_assert!(result.is_ok(), "Implementation must initialize `vxrm` CSR");
        // Mirror `vxrm` into `vcsr[2:1]`, preserving `vcsr[0]` (`vxsat`)
        let old_vcsr = self
            .read_csr(VectorCsr::Vcsr.to_csr_index())
            .unwrap_or_default();
        let new_vcsr = (old_vcsr & !Reg::Type::from(0b110u8)) | (masked << 1u8);
        let result = self.write_csr(VectorCsr::Vcsr.to_csr_index(), new_vcsr);
        debug_assert!(result.is_ok(), "Implementation must initialize `vcsr` CSR");
    }

    /// Get the current vl
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn vl(&self) -> Vl {
        let vl = self
            .read_csr(VectorCsr::Vl.to_csr_index())
            .unwrap_or_default()
            .as_u64() as u32;
        // Should always be `Some()`, but can't be guaranteed here
        Vl::new(vl).unwrap_or_default()
    }

    /// Set vl.
    ///
    /// The implementation must update both its internal decoded cache and the raw CSR value (for
    /// reads via Zicsr, writes via Zicsr are not allowed).
    ///
    /// The default implementation ignores writes to uninitialized CSR in release mode and panics in
    /// debug.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn set_vl(&mut self, vl: Vl) {
        let result = self.write_csr(VectorCsr::Vl.to_csr_index(), Reg::Type::from(u32::from(vl)));
        debug_assert!(result.is_ok(), "Implementation must initialize `vl` CSR");
    }

    /// Get the current decoded vtype
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn vtype(&self) -> Option<Vtype<{ Self::ELEN }, { Self::VLEN }>> {
        self.read_csr(VectorCsr::Vtype.to_csr_index())
            .ok()
            .and_then(Vtype::from_raw::<Reg>)
    }

    /// Set the vtype register from a decoded `Vtype`.
    ///
    /// The implementation must update both its internal decoded cache and the raw CSR value (for
    /// reads via Zicsr, writes via Zicsr are not allowed).
    ///
    /// The default implementation ignores writes to uninitialized CSR in release mode and panics in
    /// debug.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn set_vtype(&mut self, vtype: Option<Vtype<{ Self::ELEN }, { Self::VLEN }>>) {
        let vtype_raw = if let Some(vt) = vtype {
            vt.to_raw::<Reg>()
        } else {
            Vtype::<{ Self::ELEN }, { Self::VLEN }>::illegal_raw::<Reg>()
        };

        let result = self.write_csr(VectorCsr::Vtype.to_csr_index(), vtype_raw);
        debug_assert!(result.is_ok(), "Implementation must initialize `vtype` CSR");
    }
}

// Convenience for threaded execution
// TODO: Forward generically instead, once the compiler normalizes
//  `<&mut T as VectorRegisters>::VLEN` to `T::VLEN`:
//  https://github.com/rust-lang/rust/issues/161264
#[macro_export]
macro_rules! impl_vector_registers_for_mut_ref {
    ($ext_state:ty, $reg:ty) => {
        impl VectorRegisters for &mut $ext_state {
            const ELEN: Elen = <$ext_state as VectorRegisters>::ELEN;
            const VLEN: Vlen = <$ext_state as VectorRegisters>::VLEN;

            #[inline(always)]
            fn read_vregs(&self) -> &VectorRegisterFile<{ Self::VLEN }> {
                <$ext_state as VectorRegisters>::read_vregs(self)
            }

            #[inline(always)]
            fn write_vregs(&mut self) -> &mut VectorRegisterFile<{ Self::VLEN }> {
                <$ext_state as VectorRegisters>::write_vregs(self)
            }

            #[inline(always)]
            fn vector_instructions_allowed(&self) -> bool {
                <$ext_state as VectorRegisters>::vector_instructions_allowed(self)
            }

            #[inline(always)]
            fn mark_vs_dirty(&mut self) {
                <$ext_state as VectorRegisters>::mark_vs_dirty(self);
            }
        }

        impl VectorRegistersExt<$reg> for &mut $ext_state {}
    };
}
