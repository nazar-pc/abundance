#![feature(bigint_helper_methods)]
#![no_std]

#[cfg(test)]
mod tests;

use ab_riscv_primitives::instruction::{GenericInstruction, Rv64Instruction};
use ab_riscv_primitives::registers::{GenericRegister, GenericRegisters};
use core::fmt;
use core::ops::ControlFlow;

/// Errors for [`VirtualMemory`]
#[derive(Debug, thiserror::Error)]
pub enum VirtualMemoryError {
    /// Out-of-bounds read
    #[error("Out-of-bounds read at address {address}")]
    OutOfBoundsRead {
        /// Address of the out-of-bounds read
        address: u64,
    },
    /// Out-of-bounds write
    #[error("Out-of-bounds write at address {address}")]
    OutOfBoundsWrite {
        /// Address of the out-of-bounds write
        address: u64,
    },
}

mod private {
    pub trait Sealed {}

    impl Sealed for u8 {}
    impl Sealed for u16 {}
    impl Sealed for u32 {}
    impl Sealed for u64 {}
    impl Sealed for i8 {}
    impl Sealed for i16 {}
    impl Sealed for i32 {}
    impl Sealed for i64 {}
}

/// Basic integer types that can be read and written to/from memory freely
pub trait BasicInt: Sized + Copy + private::Sealed {}

impl BasicInt for u8 {}
impl BasicInt for u16 {}
impl BasicInt for u32 {}
impl BasicInt for u64 {}
impl BasicInt for i8 {}
impl BasicInt for i16 {}
impl BasicInt for i32 {}
impl BasicInt for i64 {}

/// Virtual memory interface
pub trait VirtualMemory {
    /// Read a value from memory at the specified address
    fn read<T>(&self, address: u64) -> Result<T, VirtualMemoryError>
    where
        T: BasicInt;

    /// Write a value to memory at the specified address
    fn write<T>(&mut self, address: u64, value: T) -> Result<(), VirtualMemoryError>
    where
        T: BasicInt;
}

/// Errors for [`execute_rv64()`]
#[derive(Debug, thiserror::Error)]
pub enum ExecuteError<Instruction, Custom = &'static str>
where
    Instruction: fmt::Display,
    Custom: fmt::Display,
{
    /// Unaligned instruction fetch
    #[error("Unaligned instruction fetch at address {address}")]
    UnalignedInstructionFetch {
        /// Address of the unaligned instruction fetch
        address: u64,
    },
    /// Memory access error
    #[error("Memory access error: {0}")]
    MemoryAccess(#[from] VirtualMemoryError),
    /// Unsupported instruction
    #[error("Unsupported instruction at address {address:#x}: {instruction}")]
    UnsupportedInstruction {
        /// Address of the unsupported instruction
        address: u64,
        /// Instruction that caused the error
        instruction: Instruction,
    },
    /// Unimplemented/illegal instruction
    #[error("Unimplemented/illegal instruction at address {address:#x}")]
    UnimpInstruction {
        /// Address of the `unimp` instruction
        address: u64,
    },
    /// Invalid instruction
    #[error("Invalid instruction at address {address:#x}: {instruction:#010x}")]
    InvalidInstruction {
        /// Address of the invalid instruction
        address: u64,
        /// Instruction that caused the error
        instruction: u32,
    },
    /// Custom error
    #[error("Custom error: {0}")]
    Custom(Custom),
}

/// Result of [`GenericInstructionHandler::fetch_instruction()`] call
#[derive(Debug, Copy, Clone)]
pub enum FetchInstructionResult<Instruction> {
    /// Instruction fetched successfully
    Instruction(Instruction),
    /// Control flow instruction encountered
    ControlFlow(ControlFlow<()>),
}

/// Custom handlers for instructions `ecall` and `ebreak`
pub trait GenericInstructionHandler<Instruction, Registers, Memory, CustomError>
where
    Instruction: GenericInstruction,
    CustomError: fmt::Display,
{
    /// Fetch a single instruction at a specified address and advance the program counter
    fn fetch_instruction(
        &mut self,
        _regs: &mut Registers,
        memory: &mut Memory,
        pc: &mut u64,
    ) -> Result<FetchInstructionResult<Instruction>, ExecuteError<Instruction, CustomError>>;

    /// Handle an `ecall` instruction.
    ///
    /// NOTE: the program counter here is the current value, meaning it is already incremented past
    /// the instruction itself.
    fn handle_ecall(
        &mut self,
        regs: &mut Registers,
        memory: &mut Memory,
        pc: &mut u64,
        instruction: Instruction,
    ) -> Result<(), ExecuteError<Instruction, CustomError>>;

    /// Handle an `ebreak` instruction.
    ///
    /// NOTE: the program counter here is the current value, meaning it is already incremented past
    /// the instruction itself.
    #[inline(always)]
    fn handle_ebreak(
        &mut self,
        _regs: &mut Registers,
        _memory: &mut Memory,
        _pc: &mut u64,
        _instruction: Instruction,
    ) -> Result<(), ExecuteError<Instruction, CustomError>> {
        // NOP by default
        Ok(())
    }
}

/// Basic instruction handler implementation.
///
/// `RETURN_TRAP_ADDRESS` is the address at which the interpreter will stop execution (gracefully).
#[derive(Debug, Default, Copy, Clone)]
pub struct BasicInstructionHandler<const RETURN_TRAP_ADDRESS: u64>;

impl<const RETURN_TRAP_ADDRESS: u64, Instruction, Registers, Memory>
    GenericInstructionHandler<Instruction, Registers, Memory, &'static str>
    for BasicInstructionHandler<RETURN_TRAP_ADDRESS>
where
    Instruction: GenericInstruction,
    Memory: VirtualMemory,
{
    #[inline(always)]
    fn fetch_instruction(
        &mut self,
        _regs: &mut Registers,
        memory: &mut Memory,
        pc: &mut u64,
    ) -> Result<FetchInstructionResult<Instruction>, ExecuteError<Instruction, &'static str>> {
        let address = *pc;

        if address == RETURN_TRAP_ADDRESS {
            return Ok(FetchInstructionResult::ControlFlow(ControlFlow::Break(())));
        }

        if !address.is_multiple_of(size_of::<u32>() as u64) {
            return Err(ExecuteError::UnalignedInstructionFetch { address });
        }

        let instruction = memory.read(address)?;
        let instruction = Instruction::decode(instruction);
        *pc += instruction.size() as u64;

        Ok(FetchInstructionResult::Instruction(instruction))
    }

    #[inline(always)]
    fn handle_ecall(
        &mut self,
        _regs: &mut Registers,
        _memory: &mut Memory,
        pc: &mut u64,
        instruction: Instruction,
    ) -> Result<(), ExecuteError<Instruction, &'static str>> {
        Err(ExecuteError::UnsupportedInstruction {
            address: *pc - instruction.size() as u64,
            instruction,
        })
    }
}

/// Execute RV64 instructions
pub fn execute_rv64<Reg, Registers, Memory, InstructionHandler, CustomError>(
    regs: &mut Registers,
    memory: &mut Memory,
    pc: &mut u64,
    instruction_handlers: &mut InstructionHandler,
) -> Result<(), ExecuteError<Rv64Instruction<Reg>, CustomError>>
where
    Reg: GenericRegister,
    Registers: GenericRegisters<Reg>,
    Memory: VirtualMemory,
    InstructionHandler:
        GenericInstructionHandler<Rv64Instruction<Reg>, Registers, Memory, CustomError>,
    CustomError: fmt::Display,
{
    loop {
        let old_pc = *pc;
        let instruction = match instruction_handlers.fetch_instruction(regs, memory, pc)? {
            FetchInstructionResult::Instruction(instruction) => instruction,
            FetchInstructionResult::ControlFlow(ControlFlow::Continue(())) => {
                continue;
            }
            FetchInstructionResult::ControlFlow(ControlFlow::Break(())) => {
                break;
            }
        };

        match instruction {
            Rv64Instruction::Add { rd, rs1, rs2 } => {
                let value = regs.read(rs1).wrapping_add(regs.read(rs2));
                regs.write(rd, value);
            }
            Rv64Instruction::Sub { rd, rs1, rs2 } => {
                let value = regs.read(rs1).wrapping_sub(regs.read(rs2));
                regs.write(rd, value);
            }
            Rv64Instruction::Sll { rd, rs1, rs2 } => {
                let shamt = regs.read(rs2) & 0x3f;
                let value = regs.read(rs1) << shamt;
                regs.write(rd, value);
            }
            Rv64Instruction::Slt { rd, rs1, rs2 } => {
                let value = regs.read(rs1).cast_signed() < regs.read(rs2).cast_signed();
                regs.write(rd, value as u64);
            }
            Rv64Instruction::Sltu { rd, rs1, rs2 } => {
                let value = regs.read(rs1) < regs.read(rs2);
                regs.write(rd, value as u64);
            }
            Rv64Instruction::Xor { rd, rs1, rs2 } => {
                let value = regs.read(rs1) ^ regs.read(rs2);
                regs.write(rd, value);
            }
            Rv64Instruction::Srl { rd, rs1, rs2 } => {
                let shamt = regs.read(rs2) & 0x3f;
                let value = regs.read(rs1) >> shamt;
                regs.write(rd, value);
            }
            Rv64Instruction::Sra { rd, rs1, rs2 } => {
                let shamt = regs.read(rs2) & 0x3f;
                let value = regs.read(rs1).cast_signed() >> shamt;
                regs.write(rd, value.cast_unsigned());
            }
            Rv64Instruction::Or { rd, rs1, rs2 } => {
                let value = regs.read(rs1) | regs.read(rs2);
                regs.write(rd, value);
            }
            Rv64Instruction::And { rd, rs1, rs2 } => {
                let value = regs.read(rs1) & regs.read(rs2);
                regs.write(rd, value);
            }

            Rv64Instruction::Mul { rd, rs1, rs2 } => {
                let value = regs.read(rs1).wrapping_mul(regs.read(rs2));
                regs.write(rd, value);
            }
            Rv64Instruction::Mulh { rd, rs1, rs2 } => {
                let (_lo, prod) = regs
                    .read(rs1)
                    .cast_signed()
                    .widening_mul(regs.read(rs2).cast_signed());
                regs.write(rd, prod.cast_unsigned());
            }
            Rv64Instruction::Mulhsu { rd, rs1, rs2 } => {
                let prod = (regs.read(rs1).cast_signed() as i128) * (regs.read(rs2) as i128);
                let value = prod >> 64;
                regs.write(rd, value.cast_unsigned() as u64);
            }
            Rv64Instruction::Mulhu { rd, rs1, rs2 } => {
                let prod = (regs.read(rs1) as u128) * (regs.read(rs2) as u128);
                let value = prod >> 64;
                regs.write(rd, value as u64);
            }
            Rv64Instruction::Div { rd, rs1, rs2 } => {
                let dividend = regs.read(rs1).cast_signed();
                let divisor = regs.read(rs2).cast_signed();
                let value = if divisor == 0 {
                    -1i64
                } else if dividend == i64::MIN && divisor == -1 {
                    i64::MIN
                } else {
                    dividend / divisor
                };
                regs.write(rd, value.cast_unsigned());
            }
            Rv64Instruction::Divu { rd, rs1, rs2 } => {
                let dividend = regs.read(rs1);
                let divisor = regs.read(rs2);
                let value = if divisor == 0 {
                    u64::MAX
                } else {
                    dividend / divisor
                };
                regs.write(rd, value);
            }
            Rv64Instruction::Rem { rd, rs1, rs2 } => {
                let dividend = regs.read(rs1).cast_signed();
                let divisor = regs.read(rs2).cast_signed();
                let value = if divisor == 0 {
                    dividend
                } else if dividend == i64::MIN && divisor == -1 {
                    0
                } else {
                    dividend % divisor
                };
                regs.write(rd, value.cast_unsigned());
            }
            Rv64Instruction::Remu { rd, rs1, rs2 } => {
                let dividend = regs.read(rs1);
                let divisor = regs.read(rs2);
                let value = if divisor == 0 {
                    dividend
                } else {
                    dividend % divisor
                };
                regs.write(rd, value);
            }

            Rv64Instruction::Addw { rd, rs1, rs2 } => {
                let sum = (regs.read(rs1) as i32).wrapping_add(regs.read(rs2) as i32);
                regs.write(rd, (sum as i64).cast_unsigned());
            }
            Rv64Instruction::Subw { rd, rs1, rs2 } => {
                let diff = (regs.read(rs1) as i32).wrapping_sub(regs.read(rs2) as i32);
                regs.write(rd, (diff as i64).cast_unsigned());
            }
            Rv64Instruction::Sllw { rd, rs1, rs2 } => {
                let shamt = regs.read(rs2) & 0x1f;
                let shifted = (regs.read(rs1) as u32) << shamt;
                regs.write(rd, (shifted.cast_signed() as i64).cast_unsigned());
            }
            Rv64Instruction::Srlw { rd, rs1, rs2 } => {
                let shamt = regs.read(rs2) & 0x1f;
                let shifted = (regs.read(rs1) as u32) >> shamt;
                regs.write(rd, (shifted.cast_signed() as i64).cast_unsigned());
            }
            Rv64Instruction::Sraw { rd, rs1, rs2 } => {
                let shamt = regs.read(rs2) & 0x1f;
                let shifted = (regs.read(rs1) as i32) >> shamt;
                regs.write(rd, (shifted as i64).cast_unsigned());
            }
            Rv64Instruction::Mulw { rd, rs1, rs2 } => {
                let prod = (regs.read(rs1) as i32).wrapping_mul(regs.read(rs2) as i32);
                regs.write(rd, (prod as i64).cast_unsigned());
            }
            Rv64Instruction::Divw { rd, rs1, rs2 } => {
                let dividend = regs.read(rs1) as i32;
                let divisor = regs.read(rs2) as i32;
                let value = if divisor == 0 {
                    -1i64
                } else if dividend == i32::MIN && divisor == -1 {
                    i32::MIN as i64
                } else {
                    (dividend / divisor) as i64
                };
                regs.write(rd, value.cast_unsigned());
            }
            Rv64Instruction::Divuw { rd, rs1, rs2 } => {
                let dividend = regs.read(rs1) as u32;
                let divisor = regs.read(rs2) as u32;
                let value = if divisor == 0 {
                    u64::MAX
                } else {
                    ((dividend / divisor).cast_signed() as i64).cast_unsigned()
                };
                regs.write(rd, value);
            }
            Rv64Instruction::Remw { rd, rs1, rs2 } => {
                let dividend = regs.read(rs1) as i32;
                let divisor = regs.read(rs2) as i32;
                let value = if divisor == 0 {
                    (dividend as i64).cast_unsigned()
                } else if dividend == i32::MIN && divisor == -1 {
                    0
                } else {
                    ((dividend % divisor) as i64).cast_unsigned()
                };
                regs.write(rd, value);
            }
            Rv64Instruction::Remuw { rd, rs1, rs2 } => {
                let dividend = regs.read(rs1) as u32;
                let divisor = regs.read(rs2) as u32;
                let value = if divisor == 0 {
                    dividend.cast_signed() as i64
                } else {
                    (dividend % divisor).cast_signed() as i64
                };
                regs.write(rd, value.cast_unsigned());
            }

            Rv64Instruction::Addi { rd, rs1, imm } => {
                let value = regs.read(rs1).wrapping_add((imm as i64).cast_unsigned());
                regs.write(rd, value);
            }
            Rv64Instruction::Slti { rd, rs1, imm } => {
                let value = regs.read(rs1).cast_signed() < (imm as i64);
                regs.write(rd, value as u64);
            }
            Rv64Instruction::Sltiu { rd, rs1, imm } => {
                let value = regs.read(rs1) < ((imm as i64).cast_unsigned());
                regs.write(rd, value as u64);
            }
            Rv64Instruction::Xori { rd, rs1, imm } => {
                let value = regs.read(rs1) ^ ((imm as i64).cast_unsigned());
                regs.write(rd, value);
            }
            Rv64Instruction::Ori { rd, rs1, imm } => {
                let value = regs.read(rs1) | ((imm as i64).cast_unsigned());
                regs.write(rd, value);
            }
            Rv64Instruction::Andi { rd, rs1, imm } => {
                let value = regs.read(rs1) & ((imm as i64).cast_unsigned());
                regs.write(rd, value);
            }
            Rv64Instruction::Slli { rd, rs1, shamt } => {
                let value = regs.read(rs1) << shamt;
                regs.write(rd, value);
            }
            Rv64Instruction::Srli { rd, rs1, shamt } => {
                let value = regs.read(rs1) >> shamt;
                regs.write(rd, value);
            }
            Rv64Instruction::Srai { rd, rs1, shamt } => {
                let value = regs.read(rs1).cast_signed() >> shamt;
                regs.write(rd, value.cast_unsigned());
            }

            Rv64Instruction::Addiw { rd, rs1, imm } => {
                let sum = (regs.read(rs1) as i32).wrapping_add(imm);
                regs.write(rd, (sum as i64).cast_unsigned());
            }
            Rv64Instruction::Slliw { rd, rs1, shamt } => {
                let shifted = (regs.read(rs1) as u32) << shamt;
                regs.write(rd, (shifted.cast_signed() as i64).cast_unsigned());
            }
            Rv64Instruction::Srliw { rd, rs1, shamt } => {
                let shifted = (regs.read(rs1) as u32) >> shamt;
                regs.write(rd, (shifted.cast_signed() as i64).cast_unsigned());
            }
            Rv64Instruction::Sraiw { rd, rs1, shamt } => {
                let shifted = (regs.read(rs1) as i32) >> shamt;
                regs.write(rd, (shifted as i64).cast_unsigned());
            }

            Rv64Instruction::Lb { rd, rs1, imm } => {
                let addr = regs.read(rs1).wrapping_add((imm as i64).cast_unsigned());
                let value = memory.read::<i8>(addr)? as i64;
                regs.write(rd, value.cast_unsigned());
            }
            Rv64Instruction::Lh { rd, rs1, imm } => {
                let addr = regs.read(rs1).wrapping_add((imm as i64).cast_unsigned());
                let value = memory.read::<i16>(addr)? as i64;
                regs.write(rd, value.cast_unsigned());
            }
            Rv64Instruction::Lw { rd, rs1, imm } => {
                let addr = regs.read(rs1).wrapping_add((imm as i64).cast_unsigned());
                let value = memory.read::<i32>(addr)? as i64;
                regs.write(rd, value.cast_unsigned());
            }
            Rv64Instruction::Ld { rd, rs1, imm } => {
                let addr = regs.read(rs1).wrapping_add((imm as i64).cast_unsigned());
                let value = memory.read::<u64>(addr)?;
                regs.write(rd, value);
            }
            Rv64Instruction::Lbu { rd, rs1, imm } => {
                let addr = regs.read(rs1).wrapping_add((imm as i64).cast_unsigned());
                let value = memory.read::<u8>(addr)?;
                regs.write(rd, value as u64);
            }
            Rv64Instruction::Lhu { rd, rs1, imm } => {
                let addr = regs.read(rs1).wrapping_add((imm as i64).cast_unsigned());
                let value = memory.read::<u16>(addr)?;
                regs.write(rd, value as u64);
            }
            Rv64Instruction::Lwu { rd, rs1, imm } => {
                let addr = regs.read(rs1).wrapping_add((imm as i64).cast_unsigned());
                let value = memory.read::<u32>(addr)?;
                regs.write(rd, value as u64);
            }

            Rv64Instruction::Jalr { rd, rs1, imm } => {
                let target = (regs.read(rs1).wrapping_add((imm as i64).cast_unsigned())) & !1u64;
                regs.write(rd, *pc);
                *pc = target;
            }

            Rv64Instruction::Sb { rs2, rs1, imm } => {
                let addr = regs.read(rs1).wrapping_add((imm as i64).cast_unsigned());
                memory.write(addr, regs.read(rs2) as u8)?;
            }
            Rv64Instruction::Sh { rs2, rs1, imm } => {
                let addr = regs.read(rs1).wrapping_add((imm as i64).cast_unsigned());
                memory.write(addr, regs.read(rs2) as u16)?;
            }
            Rv64Instruction::Sw { rs2, rs1, imm } => {
                let addr = regs.read(rs1).wrapping_add((imm as i64).cast_unsigned());
                memory.write(addr, regs.read(rs2) as u32)?;
            }
            Rv64Instruction::Sd { rs2, rs1, imm } => {
                let addr = regs.read(rs1).wrapping_add((imm as i64).cast_unsigned());
                memory.write(addr, regs.read(rs2))?;
            }

            Rv64Instruction::Beq { rs1, rs2, imm } => {
                if regs.read(rs1) == regs.read(rs2) {
                    *pc = old_pc.wrapping_add((imm as i64).cast_unsigned());
                }
            }
            Rv64Instruction::Bne { rs1, rs2, imm } => {
                if regs.read(rs1) != regs.read(rs2) {
                    *pc = old_pc.wrapping_add((imm as i64).cast_unsigned());
                }
            }
            Rv64Instruction::Blt { rs1, rs2, imm } => {
                if regs.read(rs1).cast_signed() < regs.read(rs2).cast_signed() {
                    *pc = old_pc.wrapping_add((imm as i64).cast_unsigned());
                }
            }
            Rv64Instruction::Bge { rs1, rs2, imm } => {
                if regs.read(rs1).cast_signed() >= regs.read(rs2).cast_signed() {
                    *pc = old_pc.wrapping_add((imm as i64).cast_unsigned());
                }
            }
            Rv64Instruction::Bltu { rs1, rs2, imm } => {
                if regs.read(rs1) < regs.read(rs2) {
                    *pc = old_pc.wrapping_add((imm as i64).cast_unsigned());
                }
            }
            Rv64Instruction::Bgeu { rs1, rs2, imm } => {
                if regs.read(rs1) >= regs.read(rs2) {
                    *pc = old_pc.wrapping_add((imm as i64).cast_unsigned());
                }
            }

            Rv64Instruction::Lui { rd, imm } => {
                regs.write(rd, (imm as i64).cast_unsigned());
            }

            Rv64Instruction::Auipc { rd, imm } => {
                regs.write(rd, old_pc.wrapping_add((imm as i64).cast_unsigned()));
            }

            Rv64Instruction::Jal { rd, imm } => {
                regs.write(rd, *pc);
                *pc = old_pc.wrapping_add((imm as i64).cast_unsigned());
            }

            Rv64Instruction::Fence { .. } => {
                // NOP for single-threaded
            }

            Rv64Instruction::Ecall => {
                instruction_handlers.handle_ecall(regs, memory, pc, Rv64Instruction::Ecall)?;
            }
            Rv64Instruction::Ebreak => {
                instruction_handlers.handle_ebreak(regs, memory, pc, Rv64Instruction::Ebreak)?;
            }

            Rv64Instruction::Unimp => {
                return Err(ExecuteError::UnimpInstruction { address: old_pc });
            }

            Rv64Instruction::Invalid(instruction) => {
                return Err(ExecuteError::InvalidInstruction {
                    address: old_pc,
                    instruction,
                });
            }
        }
    }

    Ok(())
}
