use crate::instruction::MachineModePlaceholder;
use crate::interpreter::{CoreConfig, MISA_I, MISA_M};
use ab_riscv_interpreter::prelude::*;
use ab_riscv_macros::{instruction, instruction_execution};
use ab_riscv_primitives::prelude::*;
use core::fmt;
use core::ops::ControlFlow;

/// Configuration of the RV32I core with Zve32x and Zvbb.
///
/// M is also enabled because ACT4 requires it for all vector tests.
pub(crate) const ABUNDANCE_RV32I_ZVE32X_ZVBB_CONFIG: CoreConfig = CoreConfig {
    misa_extensions: MISA_I | MISA_M,
    zkr: false,
    zca: false,
};

/// RV32I base ISA with Zve32x and Zvbb extensions
pub(crate) type AbundanceRv32IZve32xZvbbInstruction =
    AbundanceRv32IZve32xZvbbInstructionPrototype<Reg<u32>>;

/// RV32I base ISA with Zve32x and Zvbb extensions
#[instruction(
    inherit = [
        Rv32Instruction,
        Rv32MInstruction,
        ZicsrInstruction,
        ZvbbInstruction,
        ZveXxInstruction,
        MachineModePlaceholder,
    ],
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AbundanceRv32IZve32xZvbbInstructionPrototype<Reg> {}

#[instruction]
const impl<Reg> Instruction for AbundanceRv32IZve32xZvbbInstructionPrototype<Reg> {
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
impl<Reg> fmt::Display for AbundanceRv32IZve32xZvbbInstructionPrototype<Reg>
where
    Reg: Register,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

#[instruction_execution]
impl<Reg> ExecutableInstructionOperands for AbundanceRv32IZve32xZvbbInstructionPrototype<Reg> {}

#[instruction_execution]
#[expect(
    clippy::useless_conversion,
    reason = "https://github.com/rust-lang/rust-clippy/issues/17083"
)]
impl<Reg, Env> ExecutableInstructionCsr<Env> for AbundanceRv32IZve32xZvbbInstructionPrototype<Reg> {}

#[instruction_execution]
impl<Reg, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
    for AbundanceRv32IZve32xZvbbInstructionPrototype<Reg>
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
