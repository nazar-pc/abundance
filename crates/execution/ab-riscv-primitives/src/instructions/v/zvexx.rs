//! ZveXx extension (Vector Extension for Embedded Processors, ELEN=64, integer-only)

#[doc(hidden)]
pub mod arith;
#[doc(hidden)]
pub mod carry;
#[doc(hidden)]
pub mod config;
#[doc(hidden)]
pub mod fixed_point;
#[doc(hidden)]
pub mod load;
#[doc(hidden)]
pub mod mask;
#[doc(hidden)]
pub mod muldiv;
#[doc(hidden)]
pub mod perm;
#[doc(hidden)]
pub mod reduction;
#[doc(hidden)]
pub mod store;
#[doc(hidden)]
pub mod widen_narrow;

use crate::hart::{HartConfig, VectorHartConfig, VectorLengths};
use crate::instructions::v::zvexx::arith::ZveXxArithInstruction;
use crate::instructions::v::zvexx::carry::ZveXxCarryInstruction;
use crate::instructions::v::zvexx::config::ZveXxConfigInstruction;
use crate::instructions::v::zvexx::fixed_point::ZveXxFixedPointInstruction;
use crate::instructions::v::zvexx::load::{LoadStoreNreg, Nf, SegVmNf, ZveXxLoadInstruction};
use crate::instructions::v::zvexx::mask::ZveXxMaskInstruction;
use crate::instructions::v::zvexx::muldiv::ZveXxMulDivInstruction;
use crate::instructions::v::zvexx::perm::ZveXxPermInstruction;
use crate::instructions::v::zvexx::reduction::ZveXxReductionInstruction;
use crate::instructions::v::zvexx::store::ZveXxStoreInstruction;
use crate::instructions::v::zvexx::widen_narrow::ZveXxWidenNarrowInstruction;
use crate::instructions::v::{Eew, Elen, V, Vlen};
use crate::instructions::zicsr::ZicsrInstruction;
use crate::instructions::{ImplementedExtension, Instruction, IsaExtension};
use crate::registers::general_purpose::Register;
use crate::registers::vector::VReg;
use ab_riscv_macros::instruction;
use core::fmt;

/// `zvl*b` extensions for each vector length
const ZVL_EXTENSIONS: [(Vlen, &str); 12] = [
    (Vlen::L32, "zvl32b"),
    (Vlen::L64, "zvl64b"),
    (Vlen::L128, "zvl128b"),
    (Vlen::L256, "zvl256b"),
    (Vlen::L512, "zvl512b"),
    (Vlen::L1024, "zvl1024b"),
    (Vlen::L2048, "zvl2048b"),
    (Vlen::L4096, "zvl4096b"),
    (Vlen::L8192, "zvl8192b"),
    (Vlen::L16_384, "zvl16384b"),
    (Vlen::L32_768, "zvl32768b"),
    (Vlen::L65_536, "zvl65536b"),
];

/// ISA extensions of vector instructions for the vector lengths and the number of them.
///
/// Like compilers do, in addition to `zve32x` and `zve64x` (for `ELEN = 64`), this includes
/// `zvl*b` extensions for all vector lengths up to `VLEN`.
const fn vector_isa_extensions(
    vector_lengths: VectorLengths,
) -> ([IsaExtension; const { ZVL_EXTENSIONS.len() + 2 }], usize) {
    // `zve32x` is always present, the rest of the array is overwritten below
    let mut isa_extensions =
        [IsaExtension::new("zve32x", 1, 0); const { ZVL_EXTENSIONS.len() + 2 }];
    let mut num_isa_extensions = 1;
    if matches!(vector_lengths.elen, Elen::L64) {
        isa_extensions[num_isa_extensions] = IsaExtension::new("zve64x", 1, 0);
        num_isa_extensions += 1;
    }

    // For loops are not yet usable in const environment
    let mut index = 0;
    while index < ZVL_EXTENSIONS.len() {
        let (zvl_vlen, name) = ZVL_EXTENSIONS[index];
        if zvl_vlen <= vector_lengths.vlen {
            isa_extensions[num_isa_extensions] = IsaExtension::new(name, 1, 0);
            num_isa_extensions += 1;
        }
        index += 1;
    }

    (isa_extensions, num_isa_extensions)
}

/// RISC-V ZveXx instruction.
///
/// `X` is any legal value, according to the RISC-V specification, for example, Zve32x or Zve64x.
/// The actual `ELEN` and `VLEN` values are configured by the hart configuration, `ELEN` above 64
/// is not supported and doesn't compile.
#[instruction(
    inherit = [
        ZveXxConfigInstruction,
        ZveXxLoadInstruction,
        ZveXxStoreInstruction,
        ZveXxArithInstruction,
        ZveXxCarryInstruction,
        ZveXxMulDivInstruction,
        ZveXxWidenNarrowInstruction,
        ZveXxFixedPointInstruction,
        ZveXxMaskInstruction,
        ZveXxReductionInstruction,
        ZveXxPermInstruction,
        ZicsrInstruction,
    ],
)]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
pub enum ZveXxInstruction<Hart>
where
    Hart: HartConfig, {}

#[instruction]
const impl<Reg, Hart> Instruction for ZveXxInstruction<Hart>
where
    Reg: [const] Register,
    Hart: [const] VectorHartConfig<Reg = Reg>,
{
    const OWN_ISA_EXTENSIONS: &'static [IsaExtension] = {
        assert!(
            matches!(Hart::VECTOR_LENGTHS.elen, Elen::L32 | Elen::L64),
            "Zve* extensions require `ELEN` of 32 or 64 bits"
        );
        let (isa_extensions, num_isa_extensions) =
            &const { vector_isa_extensions(Hart::VECTOR_LENGTHS) };
        isa_extensions.split_at(*num_isa_extensions).0
    };

    const ALIGNMENT: u8 = align_of::<u32>() as u8;

    type Hart = Hart;

    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn try_decode(instruction: u32) -> Option<Self> {
        None
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}

#[instruction]
impl<Reg, Hart> fmt::Display for ZveXxInstruction<Hart>
where
    Reg: fmt::Display + Copy,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}
