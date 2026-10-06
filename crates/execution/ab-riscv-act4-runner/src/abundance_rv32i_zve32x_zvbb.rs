use crate::instruction::MachineModePlaceholder;
use crate::interpreter::{CoreConfig, MISA_I, MISA_M};
use ab_riscv_interpreter::prelude::*;
use ab_riscv_macros::{instruction, instruction_execution};
use ab_riscv_primitives::prelude::*;
use core::fmt;
use core::ops::ControlFlow;

/// Configuration of the RV32I core with Zve32x and Zvbb.
///
/// M is also enabled because ACT4 requires it for all vector tests, Zca is enabled because ACT4's
/// SsstrictSm tests are broken on cores without it, Zifencei is enabled because the Sail reference
/// model executes `fence.i` even when it is configured as unsupported.
pub(crate) const ABUNDANCE_RV32I_ZVE32X_ZVBB_CONFIG: CoreConfig = CoreConfig {
    misa_extensions: MISA_I | MISA_M,
    zkr: false,
    // TODO: Remove Zca once https://github.com/riscv/riscv-arch-test/issues/2584 is resolved
    zca: true,
};

/// RV32I base ISA with Zve32x and Zvbb extensions
pub(crate) type AbundanceRv32IZve32xZvbbInstruction =
    AbundanceRv32IZve32xZvbbInstructionPrototype<BasicHart<Reg<u32>>>;

/// RV32I base ISA with Zve32x and Zvbb extensions
#[instruction(
    inherit = [
        Rv32Instruction,
        Rv32MInstruction,
        // TODO: Remove Zca once https://github.com/riscv/riscv-arch-test/issues/2584 is resolved
        Rv32ZcaInstruction,
        ZicsrInstruction,
        // Zifencei is enabled because the Sail reference model executes `fence.i` even when it is
        // configured as unsupported
        // TODO: Remove Zifencei once https://github.com/riscv/sail-riscv/issues/1984 is resolved
        ZifenceiInstruction,
        ZvbbInstruction,
        ZveXxInstruction,
        MachineModePlaceholder,
    ],
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AbundanceRv32IZve32xZvbbInstructionPrototype<Hart>
where
    Hart: HartConfig, {}

#[instruction]
const impl<Reg, Hart> Instruction for AbundanceRv32IZve32xZvbbInstructionPrototype<Hart>
where
    Hart: [const] HartConfig<Reg = Reg>,
{
    const ALIGNMENT: u8 = align_of::<u32>() as u8;

    type Hart = Hart;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        None
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}

#[instruction]
impl<Reg, Hart> fmt::Display for AbundanceRv32IZve32xZvbbInstructionPrototype<Hart>
where
    Reg: Register,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

#[instruction_execution]
impl<Reg, Hart> ExecutableInstructionOperands for AbundanceRv32IZve32xZvbbInstructionPrototype<Hart> where
    Hart: HartConfig<Reg = Reg>
{
}

#[instruction_execution]
#[expect(
    clippy::useless_conversion,
    reason = "https://github.com/rust-lang/rust-clippy/issues/17083"
)]
impl<Reg, Hart, Env> ExecutableInstructionCsr<Env>
    for AbundanceRv32IZve32xZvbbInstructionPrototype<Hart>
where
    Hart: HartConfig<Reg = Reg>,
{
}

#[instruction_execution]
impl<Reg, Hart, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
    for AbundanceRv32IZve32xZvbbInstructionPrototype<Hart>
where
    Reg: Register,
    Hart: HartConfig<Reg = Reg>,
{
    fn execute(
        self,
        Rs1Rs2OperandValues {
            rs1_value,
            rs2_value,
        }: Rs1Rs2OperandValues<Reg::Type>,
        regs: &mut Regs,
        env: &mut Env,
        memory: &mut Memory,
        program_counter: &mut PC,
    ) -> ExecutionResult<Reg> {
        ExecutionResult::ContinueNoWrite
    }
}
