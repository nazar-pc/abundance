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
/// SsstrictSm tests are broken on cores without it.
pub(crate) const ABUNDANCE_RV64I_ZVE32X_ZVBB_CONFIG: CoreConfig = CoreConfig {
    misa_extensions: MISA_I | MISA_M,
    zkr: false,
    // TODO: Remove Zca once https://github.com/riscv/riscv-arch-test/issues/2584 is resolved
    zca: true,
};

/// RV64I base ISA with Zve32x and Zvbb extensions
pub(crate) type AbundanceRv64IZve32xZvbbInstruction =
    AbundanceRv64IZve32xZvbbInstructionPrototype<Reg<u64>>;

/// RV64I base ISA with Zve32x and Zvbb extensions
#[instruction(
    inherit = [
        Rv64Instruction,
        Rv64MInstruction,
        // TODO: Remove Zca once https://github.com/riscv/riscv-arch-test/issues/2584 is resolved
        Rv64ZcaInstruction,
        ZicsrInstruction,
        ZvbbInstruction,
        ZveXxInstruction,
        MachineModePlaceholder,
    ],
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AbundanceRv64IZve32xZvbbInstructionPrototype<Reg> {}

#[instruction]
const impl<Reg> Instruction for AbundanceRv64IZve32xZvbbInstructionPrototype<Reg> {
    const ALIGNMENT: u8 = align_of::<u32>() as u8;

    type Reg = Reg;

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
impl<Reg> fmt::Display for AbundanceRv64IZve32xZvbbInstructionPrototype<Reg>
where
    Reg: Register,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

#[instruction_execution]
impl<Reg> ExecutableInstructionOperands for AbundanceRv64IZve32xZvbbInstructionPrototype<Reg> {}

#[instruction_execution]
impl<Reg, Env> ExecutableInstructionCsr<Env> for AbundanceRv64IZve32xZvbbInstructionPrototype<Reg> {}

#[instruction_execution]
impl<Reg, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
    for AbundanceRv64IZve32xZvbbInstructionPrototype<Reg>
where
    Reg: Register,
{
    fn execute(
        self,
        Rs1Rs2OperandValues {
            rs1_value,
            rs2_value,
        }: Rs1Rs2OperandValues<<Self::Reg as Register>::Type>,
        regs: &mut Regs,
        env: &mut Env,
        memory: &mut Memory,
        program_counter: &mut PC,
    ) -> ExecutionResult<Self::Reg> {
        ExecutionResult::ContinueNoWrite
    }
}
