use crate::instruction::MachineModePlaceholder;
use crate::interpreter::{CoreConfig, MISA_I, MISA_M};
use ab_riscv_interpreter::prelude::*;
use ab_riscv_macros::{instruction, instruction_execution};
use ab_riscv_primitives::prelude::*;
use core::fmt;
use core::ops::ControlFlow;

/// Configuration of the RV64I core with Zve32x and Zvbb.
///
/// M is also enabled because ACT4 requires it for all vector tests, Zca is enabled because ACT4's
/// SsstrictSm tests are broken on cores without it, Zifencei is enabled because the Sail reference
/// model executes `fence.i` even when it is configured as unsupported.
pub(crate) const ABUNDANCE_RV64I_ZVE32X_ZVBB_CONFIG: CoreConfig = CoreConfig {
    misa_extensions: MISA_I | MISA_M,
    zkr: false,
    // TODO: Remove Zca once https://github.com/riscv/riscv-arch-test/issues/2584 is resolved
    zca: true,
};

/// RV64I base ISA with Zve32x and Zvbb extensions
pub(crate) type AbundanceRv64IZve32xZvbbInstruction = AbundanceRv64IZve32xZvbbInstructionPrototype<
    BasicVectorHart<Reg<u64>, { Elen::L32 }, { Vlen::L128 }>,
>;

/// RV64I base ISA with Zve32x and Zvbb extensions
#[instruction(
    inherit = [
        Rv64Instruction,
        Rv64MInstruction,
        // TODO: Remove Zca once https://github.com/riscv/riscv-arch-test/issues/2584 is resolved
        Rv64ZcaInstruction,
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
pub(crate) enum AbundanceRv64IZve32xZvbbInstructionPrototype<Hart>
where
    Hart: HartConfig, {}

#[instruction]
const impl<Reg, Hart> Instruction for AbundanceRv64IZve32xZvbbInstructionPrototype<Hart>
where
    Hart: [const] VectorHartConfig<Reg = Reg>,
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
impl<Reg, Hart> fmt::Display for AbundanceRv64IZve32xZvbbInstructionPrototype<Hart>
where
    Reg: Register,
    Hart: VectorHartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

#[instruction_execution]
impl<Reg, Hart> ExecutableInstructionOperands for AbundanceRv64IZve32xZvbbInstructionPrototype<Hart> where
    Hart: HartConfig<Reg = Reg>
{
}

#[instruction_execution]
impl<Reg, Hart, Env> ExecutableInstructionCsr<Env>
    for AbundanceRv64IZve32xZvbbInstructionPrototype<Hart>
where
    Hart: VectorHartConfig<Reg = Reg>,
{
}

#[instruction_execution]
impl<Reg, Hart, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
    for AbundanceRv64IZve32xZvbbInstructionPrototype<Hart>
where
    Reg: Register,
    Hart: VectorHartConfig<Reg = Reg>,
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
