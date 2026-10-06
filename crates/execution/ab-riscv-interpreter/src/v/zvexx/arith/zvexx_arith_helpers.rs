//! Opaque helpers for ZveXx extension

use crate::v::vector_config::BoundedVl;
use crate::v::vector_registers::{VRegGroup, VectorRegisterFile, VectorRegistersExt};
use crate::v::zvexx::load::zvexx_load_helpers::{mask_bit, snapshot_mask};
use crate::v::zvexx::zvexx_helpers::INSTRUCTION_SIZE;
use crate::{ExecutionError, PackedAddress, ProgramCounter};
use ab_riscv_primitives::prelude::*;
use core::hint::cold_path;

/// Effective element width of a register operand at `SEW`
const SEW_EEW<const SEW: Vsew>: Eew = SEW.as_eew();

/// Check mask-destination / source overlap constraint for compare/carry instructions.
///
/// Per RVV §5.2's narrowing-destination overlap rule (a mask destination has EEW=1, narrower
/// than any source EEW), `vd` may overlap a multi-register source group only in the
/// lowest-numbered register of that group. Overlapping any other register in the group is
/// reserved: `execute_compare_op`/`execute_carry_*_mask` process elements in increasing index
/// order and write one mask bit per element into `vd`. When `vd` is the group's base register,
/// every mask byte written during the processing of register `base_reg` targets bytes that hold
/// data from elements at or before the one just read (byte `b` can only be touched while
/// processing elements `>= 8*b`, and it stores raw data for element `b / sew_bytes <= 8*b`, which
/// is always read first). When `vd` is any later register in the group, that guarantee no longer
/// holds: early elements from the group's *first* register write mask bits into byte 0 of `vd`,
/// which is where the not-yet-read data of a later element (stored in `vd` itself) lives,
/// corrupting it before it is read.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn check_mask_dest_overlap<Reg, Env, Memory, PC>(
    program_counter: &PC,
    vd: VReg,
    src: VRegGroup<Env::Hart>,
) -> Result<(), ExecutionError<Reg::Type>>
where
    Reg: Register,
    Env: VectorRegistersExt<Hart: HartConfig<Reg = Reg>>,
    PC: ProgramCounter<Reg::Type, Memory>,
{
    if src.contains(vd) && vd != src.base() {
        cold_path();
        return Err(ExecutionError::IllegalInstruction {
            address: PackedAddress::new(program_counter.old_pc(INSTRUCTION_SIZE)),
        });
    }
    Ok(())
}

/// Write one mask bit (the comparison result for element `elem_i`) into register `vd`.
///
/// Bits are stored LSB-first: element `i` lives at byte `i / 8`, bit `i % 8`.
/// Only the target bit is modified; all other bits are undisturbed (tail-undisturbed semantics
/// required for mask destinations per spec §5.3).
///
/// Returns `None` if `elem_i` is not below `VLEN`, which never happens for `elem_i < vl`.
#[inline(always)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub(in super::super) fn write_mask_bit<Hart>(
    vregs: &mut VectorRegisterFile<Hart>,
    vd: VReg,
    elem_i: u16,
    result: bool,
) -> Option<()>
where
    Hart: VectorHartConfig,
{
    let byte_idx = usize::from(elem_i / u8::BITS as u16);
    let bit_idx = elem_i % u8::BITS as u16;
    let byte = vregs.get_mut(vd).get_mut(byte_idx)?;
    if result {
        *byte |= 1 << bit_idx;
    } else {
        *byte &= !(1 << bit_idx);
    }
    Some(())
}

/// Operand source
#[derive(Debug)]
#[doc(hidden)]
pub enum OpSrc<V> {
    /// Vector-vector: source register group
    Vreg(V),
    /// Vector-scalar: scalar value (sign- or zero-extended to u64)
    Scalar(u64),
}

impl<Hart> OpSrc<VRegGroup<Hart>>
where
    Hart: VectorHartConfig,
{
    /// The same source if a vector register group source has `vl` of `vl`, `None` otherwise, see
    /// [`VRegGroup::with_same_vl()`]
    #[inline(always)]
    #[doc(hidden)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub fn with_same_vl(self, vl: BoundedVl<Hart>) -> Option<SameVlOpSrc<Hart>> {
        match self {
            Self::Vreg(group) => Some(SameVlOpSrc {
                vreg: Some(group.with_same_vl(vl)?),
                scalar: 0,
            }),
            Self::Scalar(scalar) => Some(SameVlOpSrc { vreg: None, scalar }),
        }
    }
}

/// [`OpSrc`] with a vector register group source of a known `vl`, see [`OpSrc::with_same_vl()`].
///
/// Unlike an enum, the group doesn't share storage with the scalar, which would otherwise hide
/// from the optimizer that the group's `vl` is the one it was rebound to.
#[derive(Debug, Clone, Copy)]
#[doc(hidden)]
pub struct SameVlOpSrc<Hart>
where
    Hart: VectorHartConfig,
{
    /// Vector-vector: source register group
    pub vreg: Option<VRegGroup<Hart>>,
    /// Vector-scalar: scalar value (sign- or zero-extended to u64), used when `vreg` is `None`
    pub scalar: u64,
}

/// Execute a single-width element-wise arithmetic operation over `0..vl`.
///
/// `op` receives `(vs2_elem: u64, src_elem: u64, sew: Vsew)` and returns the `u64` result (only the
/// low `sew.bits_width()` are written back).
///
/// All register groups must have element width `sew` and the same `vl`, which holds for groups
/// created from the same configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_arith_op<Reg, Env, F>(
    env: &mut Env,
    vd: VRegGroup<Env::Hart>,
    vs2: VRegGroup<Env::Hart>,
    src: OpSrc<VRegGroup<Env::Hart>>,
    vm: bool,
    sew: Vsew,
    op: F,
) where
    Reg: Register,
    Env: VectorRegistersExt<Hart: HartConfig<Reg = Reg>>,
    F: Fn(u64, u64, Vsew) -> u64,
{
    // Dispatch on the element width once, so that the loop below is compiled for each width
    // separately, with element loads and stores of a constant size
    match sew {
        Vsew::E8 => {
            execute_arith_op_const::<{ Vsew::E8 }, _, _, _>(env, vd, vs2, src, vm, op);
        }
        Vsew::E16 => {
            execute_arith_op_const::<{ Vsew::E16 }, _, _, _>(env, vd, vs2, src, vm, op);
        }
        Vsew::E32 => {
            execute_arith_op_const::<{ Vsew::E32 }, _, _, _>(env, vd, vs2, src, vm, op);
        }
        Vsew::E64 => {
            execute_arith_op_const::<{ Vsew::E64 }, _, _, _>(env, vd, vs2, src, vm, op);
        }
    }
}

/// [`execute_arith_op()`] with the element width known at compile time
#[inline(always)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
fn execute_arith_op_const<const SEW: Vsew, Reg, Env, F>(
    env: &mut Env,
    vd: VRegGroup<Env::Hart>,
    vs2: VRegGroup<Env::Hart>,
    src: OpSrc<VRegGroup<Env::Hart>>,
    vm: bool,
    op: F,
) where
    Reg: Register,
    Env: VectorRegistersExt<Hart: HartConfig<Reg = Reg>>,
    F: Fn(u64, u64, Vsew) -> u64,
{
    let vl = vd.vl();
    let src_eew_matches = match src {
        OpSrc::Vreg(vs1) => vs1.eew() == SEW_EEW::<SEW>,
        OpSrc::Scalar(_) => true,
    };
    if vd.eew() != SEW_EEW::<SEW> || vs2.eew() != SEW_EEW::<SEW> || !src_eew_matches {
        cold_path();
        return;
    }
    let (Some(vs2), Some(src)) = (vs2.with_same_vl(vl), src.with_same_vl(vl)) else {
        cold_path();
        return;
    };

    let vregs = env.write_vregs();

    for i in vl.indices() {
        // The decoder rejects masked `vd == v0`, so the mask can be read in place rather than
        // snapshotted, no write below can modify it
        if !vm && !mask_bit(vregs.get(VReg::V0), i) {
            continue;
        }

        let a = vregs
            .read_const::<{ SEW_EEW::<SEW> }>(vs2, i)
            .expect("`vs2` has element width `SEW` and `vl` checked above; qed");

        let b = match src.vreg {
            Some(vs1) => vregs
                .read_const::<{ SEW_EEW::<SEW> }>(vs1, i)
                .expect("`vs1` has element width `SEW` and `vl` checked above; qed"),
            None => src.scalar,
        };

        let result = op(a, b, SEW);

        vregs
            .write_const::<{ SEW_EEW::<SEW> }>(vd, i, result)
            .expect("`vd` has element width `SEW` and `vl` checked above; qed");
    }

    env.mark_vs_dirty();
}

/// Execute a single-width element-wise integer compare over `0..vl`, writing one result
/// bit per element into the mask register `vd`.
///
/// `op` receives `(vs2_elem: u64, src_elem: u64, sew: Vsew) -> bool`.
///
/// Mask destination tail bits (indices `>= vl`) are always left undisturbed per spec §5.3,
/// regardless of `vta`. Only bits in `0..vl` are written.
///
/// Source register groups must have the same `vl`, which holds for groups created from the same
/// configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_compare_op<Reg, Env, F>(
    env: &mut Env,
    vd: VReg,
    vs2: VRegGroup<Env::Hart>,
    src: OpSrc<VRegGroup<Env::Hart>>,
    vm: bool,
    sew: Vsew,
    op: F,
) where
    Reg: Register,
    Env: VectorRegistersExt<Hart: HartConfig<Reg = Reg>>,
    F: Fn(u64, u64, Vsew) -> bool,
{
    let vl = vs2.vl();
    let Some(src) = src.with_same_vl(vl) else {
        cold_path();
        return;
    };

    let mask_buf = snapshot_mask(env.read_vregs(), vm);

    for i in vl.indices() {
        // When masked, inactive elements in the destination mask register are left undisturbed
        // (spec §12.8: "mask register results follow mask-undisturbed policy")
        if !mask_bit(&mask_buf, i) {
            continue;
        }

        let a = env
            .read_vregs()
            .read(vs2, i)
            .expect("`i < vl` of `vs2`; qed");

        let b = match src.vreg {
            Some(vs1) => env
                .read_vregs()
                .read(vs1, i)
                .expect("`vs1` has the same `vl` as `vs2`, checked above; qed"),
            None => src.scalar,
        };

        let result = op(a, b, sew);

        write_mask_bit(env.write_vregs(), vd, i, result)
            .expect("`i < vl <= VLEN`, so the mask bit is within `vd`; qed");
    }

    env.mark_vs_dirty();
}

/// Sign-extend the low `sew.bits_width()` of `val` to a full `i64`
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn sign_extend(val: u64, sew: Vsew) -> i64 {
    let shift = u64::BITS - u32::from(sew.bits_width());
    (val.cast_signed() << shift) >> shift
}

/// Mask off the upper bits of a `u64` to leave only the low `sew.bits_width()`.
///
/// Used for unsigned arithmetic and comparisons where only the SEW-wide portion is significant. For
/// SEW = 64 this is a no-op (all bits are significant).
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn sew_mask(sew: Vsew) -> u64 {
    if u32::from(sew.bits_width()) == u64::BITS {
        u64::MAX
    } else {
        (1u64 << sew.bits_width()) - 1
    }
}
