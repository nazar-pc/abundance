//! RV32 Zbkb extension

#[cfg(test)]
mod tests;

use crate::{ExecutableInstruction, ExecutionError, InterpreterState};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::instructions::rv32::zk::zbkb::Rv32ZbkbInstruction;
use ab_riscv_primitives::registers::general_purpose::Register;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        InterpreterState<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for Rv32ZbkbInstruction<Reg>
where
    Reg: Register<Type = u32>,
    [(); Reg::N]:,
{
    #[inline(always)]
    fn execute(
        self,
        state: &mut InterpreterState<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, CustomError>> {
        match self {
            Self::Pack { rd, rs1, rs2 } => {
                // Pack low 16 bits of rs1 into rd[15:0],
                // low 16 bits of rs2 into rd[31:16].
                let lo = state.regs.read(rs1) & 0x0000_FFFFu32;
                let hi = (state.regs.read(rs2) & 0x0000_FFFFu32) << 16;
                state.regs.write(rd, lo | hi);
            }
            Self::Packh { rd, rs1, rs2 } => {
                // Pack low byte of rs1 into bits [7:0], low byte of rs2 into bits [15:8].
                // Upper bits of rd are zeroed.
                let lo = state.regs.read(rs1) & 0xFF;
                let hi = (state.regs.read(rs2) & 0xFF) << 8;
                state.regs.write(rd, lo | hi);
            }
            Self::Brev8 { rd, rs1 } => {
                // Reverse bits within each byte of rs1
                let src = state.regs.read(rs1);
                let bytes = src.to_le_bytes().map(u8::reverse_bits);
                state.regs.write(rd, u32::from_le_bytes(bytes));
            }
            Self::Zip { rd, rs1 } => {
                // Bit-interleave: scatter bits of rs1 so that
                // rs1[i]    -> rd[2*i]   (even positions, lower half source)
                // rs1[i+16] -> rd[2*i+1] (odd positions, upper half source)
                // for i in 0..16.
                let src = state.regs.read(rs1);

                // TODO: Miri is excluded because corresponding intrinsic is not implemented there
                let value = cfg_select! {
                    all(not(miri), target_arch = "riscv32", target_feature = "zbkb") => {
                        core::arch::riscv32::zip(src)
                    }
                    _ => {{
                        let lo = src & 0x0000_FFFF;
                        let hi = src >> 16;

                        // Spread each 16-bit half into alternating bits.
                        // Classic SWAR interleave for 16-bit -> 32-bit Morton.
                        #[inline(always)]
                        fn spread(mut x: u32) -> u32 {
                            x = (x | (x << 8)) & 0x00FF_00FF;
                            x = (x | (x << 4)) & 0x0F0F_0F0F;
                            x = (x | (x << 2)) & 0x3333_3333;
                            (x | (x << 1)) & 0x5555_5555
                        }

                        spread(lo) | (spread(hi) << 1)
                    }}
                };

                state.regs.write(rd, value);
            }
            Self::Unzip { rd, rs1 } => {
                // Inverse of zip: gather bits of rs1 so that
                // rs1[2*i]   -> rd[i]    (even-position bits -> lower half)
                // rs1[2*i+1] -> rd[i+16] (odd-position bits -> upper half)
                // for i in 0..16.
                let src = state.regs.read(rs1);

                // TODO: Miri is excluded because corresponding intrinsic is not implemented there
                let value = cfg_select! {
                    all(not(miri), target_arch = "riscv32", target_feature = "zbkb") => {
                        core::arch::riscv32::unzip(src)
                    }
                    _ => {{
                        #[inline(always)]
                        fn compact(mut x: u32) -> u32 {
                            x &= 0x5555_5555;
                            x = (x | (x >> 1)) & 0x3333_3333;
                            x = (x | (x >> 2)) & 0x0F0F_0F0F;
                            x = (x | (x >> 4)) & 0x00FF_00FF;
                            (x | (x >> 8)) & 0x0000_FFFF
                        }

                        let lo = compact(src);
                        let hi = compact(src >> 1);
                        lo | (hi << 16)
                    }}
                };

                state.regs.write(rd, value);
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}
