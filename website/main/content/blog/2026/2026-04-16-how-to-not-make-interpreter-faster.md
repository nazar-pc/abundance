---
title: How to not make interpreter faster
date: 2026-04-16
draft: false
description: Lessons learned from trying to improve RISC-V interpreter performance
tags: [ status-update ]
authors: [ nazar-pc ]
---

I like posting updates with substantial performance improvements, but sometimes that is just not happening despite
plausible approaches and substantial effort. It is still a useful learning experience to share, though. And I have
non-performance improvements to share too, so let's get to it.

<!--more-->

## New releases

Before getting to optimization efforts, I shipped new releases of [ab-riscv-primitives], [ab-riscv-interpreter] and
related macro crates. The biggest change this time is a large number of additional implemented extensions:

[ab-riscv-primitives]: https://crates.io/crates/ab-riscv-primitives

[ab-riscv-interpreter]: https://crates.io/crates/ab-riscv-interpreter

* Full support for Zk (NIST Suite) extension, which required:
    * Zbkb (Bit manipulation instructions for cryptography) - added
    * Zbkc (Carry-less multiply instructions) - already supported before
    * Zbkx (Cross-bar Permutation instructions) - added
    * Zkne (AES encryption instructions) - added
    * Zknd (AES decryption instructions) - added
    * Zknh (SHA2 hash function instructions) - already supported before
* Some compressed instructions:
    * Zca
    * Zcb
    * Zcmp
* Zicond (Integer Conditional Operations)

All extensions are implemented for both RV32 and RV64 and pass RISC-V Architectural Certification Tests except Zcmp,
which simply [doesn't have certification tests yet].

[doesn't have certification tests yet]: https://github.com/riscv/riscv-arch-test/issues/1260

Moreover, AES instructions take advantage of hardware intrinsics on x86-64 and aarch64 where possible. Of course, on
RV32 and RV64 native intrinsics are also used where possible too.

## Optimizing instruction dispatch

Previously (and still) the general workflow of the interpreter is instruction decoding followed by execution. Not only
that, the two are modular rather than fused, so first, decoding happens like this (smaller extension for shorter
examples):

```rust
impl<Reg> const Instruction for Rv32ZbaInstruction<Reg>
where
    Reg: [ const ] Register<Type=u32>,
{
    type Reg = Reg;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        let opcode = (instruction & 0b111_1111) as u8;
        let rd_bits = ((instruction >> 7) & 0x1f) as u8;
        let funct3 = ((instruction >> 12) & 0b111) as u8;
        let rs1_bits = ((instruction >> 15) & 0x1f) as u8;
        let rs2_bits = ((instruction >> 20) & 0x1f) as u8;
        let funct7 = ((instruction >> 25) & 0b111_1111) as u8;

        match opcode {
            // R-type
            0b0110011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                match (funct3, funct7) {
                    (0b010, 0b0010000) => Some(Self::Sh1add { rd, rs1, rs2 }),
                    (0b100, 0b0010000) => Some(Self::Sh2add { rd, rs1, rs2 }),
                    (0b110, 0b0010000) => Some(Self::Sh3add { rd, rs1, rs2 }),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    // ...
}

```

Here we parse the 32-bit instruction and then create enum variants for each distinct instruction. Now that instruction
is decoded, it is matched on for execution:

```rust
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
ExecutableInstruction<
    InterpreterState<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>,
    CustomError,
> for Rv32ZbaInstruction<Reg>
where
    Reg: Register<Type=u32>,
{
    #[inline(always)]
    fn execute(
        self,
        state: &mut InterpreterState<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, CustomError>> {
        match self {
            Self::Sh1add { rd, rs1, rs2 } => {
                let value = (state.regs.read(rs1) << 1).wrapping_add(state.regs.read(rs2));
                state.regs.write(rd, value);
            }
            Self::Sh2add { rd, rs1, rs2 } => {
                let value = (state.regs.read(rs1) << 2).wrapping_add(state.regs.read(rs2));
                state.regs.write(rd, value);
            }
            Self::Sh3add { rd, rs1, rs2 } => {
                let value = (state.regs.read(rs1) << 3).wrapping_add(state.regs.read(rs2));
                state.regs.write(rd, value);
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}
```

In real cases the enum is processed by macros, and there are many more variants in both decoding and execution that
correspond to the composition of the base ISA and all selected extensions, but the idea is the same.

The observation is that we match on the enum that was just decoded, which is the cost of modularity, but it still feels
wasteful. Since enums are already processed by macros, I can generate more code to eliminate the extra match. For
example, we can decode instructions into tuple `(Instruction, execution_fn)`, so instead of doing match on the enum,
execution is just a function call. Now the `execution_fn` is a function pointer, so on 64-bit platform it is 8 extra
bytes per instruction, so considering that the total number of unique instructions is limited, pointers to execution
functions can be replaced with indexing into a dense array of function pointers, reducing the overhead to 16 bits, or
possibly even 8 bits in case the number of selected instructions is small enough.

All that can be done with a bit of macro magic. For the following extra code is generated automatically:

```rust
/// Decode and map [`Rv32ZbaInstruction`].
///
/// Each method is guaranteed to receive a corresponding enum variant as `instruction`.
pub const trait Rv32ZbaInstructionMap<Reg> {
    type Output: [ const ] ::core::marker::Destruct;

    /// Decode and map [`Rv32ZbaInstruction::Sh1add`] instruction
    fn sh1add(instruction: Rv32ZbaInstruction<Reg>) -> Self::Output;
    /// Decode and map [`Rv32ZbaInstruction::Sh2add`] instruction
    fn sh2add(instruction: Rv32ZbaInstruction<Reg>) -> Self::Output;
    /// Decode and map [`Rv32ZbaInstruction::Sh3add`] instruction
    fn sh3add(instruction: Rv32ZbaInstruction<Reg>) -> Self::Output;
}

impl<Reg> Rv32ZbaInstruction<Reg> {
    ///   Decode an instruction and immediately map it
    pub const fn try_decode_and_map<Map>(instruction: u32) -> ::core::option::Option<Map::Output>
    where
        Reg: [ const ] Register<Type=u32>,
        Map: [ const ] Rv32ZbaInstructionMap<Reg>,
    {
        let opcode = (instruction & 0b111_1111) as u8;
        let rd_bits = ((instruction >> 7) & 0x1f) as u8;
        let funct3 = ((instruction >> 12) & 0b111) as u8;
        let rs1_bits = ((instruction >> 15) & 0x1f) as u8;
        let rs2_bits = ((instruction >> 20) & 0x1f) as u8;
        let funct7 = ((instruction >> 25) & 0b111_1111) as u8;
        match opcode {
            0b0110011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                match (funct3, funct7) {
                    (0b010, 0b0010000) => {
                        Some(Map::sh1add(Self::Sh1add { rd, rs1, rs2 }))
                    }
                    (0b100, 0b0010000) => {
                        Some(Map::sh2add(Self::Sh2add { rd, rs1, rs2 }))
                    }
                    (0b110, 0b0010000) => {
                        Some(Map::sh3add(Self::Sh3add { rd, rs1, rs2 }))
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }
}
}
```

Essentially, we get `*Map` trait and `try_decode_and_map()` method that instead of `Self::Variant` returns
`Map::variant(Self::Variant)` result, allowing to immediately map the instruction without matching it first.

And the execution side gains a corresponding generated mapping implementation:

```rust
/// Decode and map [`Rv32ZbaInstruction`] for execution
#[derive(Debug)]
pub struct Rv32ZbaInstructionMapExecute<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>
{
    phantom: PhantomData<(Reg, ExtState, Memory, PC, InstructionHandler, CustomError)>,
}

impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError> const Rv32ZbaInstructionMap<Reg>
for Rv32ZbaInstructionMapExecute<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>
where
    Reg: Register<Type=u32>,
    Reg: [ const ] Register,
{
    type Output = (
        Rv32ZbaInstruction<Reg>,
        fn(
            Rv32ZbaInstruction<Reg>,
            &mut InterpreterState<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>,
        ) -> Result<
            ControlFlow<()>,
            ExecutionError<<Reg as Register>::Type, CustomError>,
        >,
    );

    #[inline(always)]
    fn sh1add(
        instruction: Rv32ZbaInstruction<Reg>,
    ) -> <Self as Rv32ZbaInstructionMap<Reg>>::Output {
        (instruction, |instruction, state| {
            let __self = instruction;
            let Rv32ZbaInstruction::Sh1add { rd, rs1, rs2 } = instruction else {
                unsafe {
                    unreachable_unchecked();
                }
            };
            {
                let value = (state.regs.read(rs1) << 1).wrapping_add(state.regs.read(rs2));
                state.regs.write(rd, value);
                Ok(ControlFlow::Continue(()))
            }
        })
    }

    // ..
}

/// Mapped executable [`Rv32ZbaInstruction`] that doesn't match on instruction variant for
/// execution at the cost of a larger data structure size
pub struct MappedExecutableRv32ZbaInstruction<
    Reg,
    ExtState,
    Memory,
    PC,
    InstructionHandler,
    CustomError,
>
where
    Reg: Register<Type=u32>,
{
    instruction: Rv32ZbaInstruction<Reg>,
    execute_fn: fn(
        Rv32ZbaInstruction<Reg>,
        &mut InterpreterState<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, CustomError>>,
}

impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError> const Instruction
for MappedExecutableRv32ZbaInstruction<
    Reg,
    ExtState,
    Memory,
    PC,
    InstructionHandler,
    CustomError,
>
where
    Reg: Register<Type=u32>,
    Reg: [ const ] Register,
{
    type Reg = Reg;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        let (instruction, execute_fn) = Rv32ZbaInstruction::try_decode_and_map::<
            Rv32ZbaInstructionMapExecute<
                Reg,
                ExtState,
                Memory,
                PC,
                InstructionHandler,
                CustomError,
            >,
        >(instruction)?;
        Some(Self { instruction, execute_fn })
    }

    // ...
}

impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
ExecutableInstruction<
    InterpreterState<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>,
    CustomError,
>
for MappedExecutableRv32ZbaInstruction<
    Reg,
    ExtState,
    Memory,
    PC,
    InstructionHandler,
    CustomError,
>
where
    Reg: Register<Type=u32>,
{
    #[inline(always)]
    fn execute(
        self,
        state: &mut InterpreterState<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, CustomError>> {
        (self.execute_fn)(self.instruction, state)
    }
}
```

Now `MappedExecutableRv32ZbaInstruction` can be used in place of a regular instruction, but its execution bypasses the
`match` statement, jumping straight into logic execution.

However, when I benchmarked all three cases, the results were not positive, quite the opposite! I did some profiling and
saw the following:

```
Here is `perf stat` with initial `match` approach:
     7.010864158                  0      context-switches:u               #      0,0 cs/sec  cs_per_second     
     7.010864158                  0      cpu-migrations:u                 #      0,0 migrations/sec  migrations_per_second
     7.010864158                  0      page-faults:u                    #      0,0 faults/sec  page_faults_per_second
     7.010864158           1 004,12 msec task-clock:u                     #      1,0 CPUs  CPUs_utilized       
     7.010864158         17 233 877      branch-misses:u                  #      0,6 %  branch_miss_rate         (50,19%)
     7.010864158      2 822 333 279      branches:u                       #   2810,8 M/sec  branch_frequency     (50,19%)
     7.010864158      4 890 961 395      cpu-cycles:u                     #      4,9 GHz  cycles_frequency       (66,91%)
     7.010864158     14 510 845 517      instructions:u                   #      3,0 instructions  insn_per_cycle  (49,81%)
     7.010864158      1 079 796 046      stalled-cycles-frontend:u        #     0,22 frontend_cycles_idle        (49,81%)

Function pointers:
     7.009750022                  0      context-switches:u               #      0,0 cs/sec  cs_per_second     
     7.009750022                  0      cpu-migrations:u                 #      0,0 migrations/sec  migrations_per_second
     7.009750022                  0      page-faults:u                    #      0,0 faults/sec  page_faults_per_second
     7.009750022           1 000,71 msec task-clock:u                     #      1,0 CPUs  CPUs_utilized       
     7.009750022         62 386 430      branch-misses:u                  #      2,6 %  branch_miss_rate         (49,79%)
     7.009750022      2 406 122 728      branches:u                       #   2404,4 M/sec  branch_frequency     (49,66%)
     7.009750022      4 873 086 373      cpu-cycles:u                     #      4,9 GHz  cycles_frequency       (66,43%)
     7.009750022     12 020 645 675      instructions:u                   #      2,5 instructions  insn_per_cycle  (50,21%)
     7.009750022      1 531 688 449      stalled-cycles-frontend:u        #     0,31 frontend_cycles_idle        (50,34%)

Function pointers through intermediate array indexing using `u16` offset:
     7.010677687                  0      context-switches:u               #      0,0 cs/sec  cs_per_second     
     7.010677687                  0      cpu-migrations:u                 #      0,0 migrations/sec  migrations_per_second
     7.010677687                  0      page-faults:u                    #      0,0 faults/sec  page_faults_per_second
     7.010677687           1 004,16 msec task-clock:u                     #      1,0 CPUs  CPUs_utilized       
     7.010677687         20 015 515      branch-misses:u                  #      0,7 %  branch_miss_rate         (50,19%)
     7.010677687      2 862 855 104      branches:u                       #   2851,0 M/sec  branch_frequency     (50,19%)
     7.010677687      4 889 795 691      cpu-cycles:u                     #      4,9 GHz  cycles_frequency       (66,92%)
     7.010677687     13 933 760 493      instructions:u                   #      2,8 instructions  insn_per_cycle  (49,81%)
     7.010677687        908 685 544      stalled-cycles-frontend:u        #     0,19 frontend_cycles_idle        (49,81%)
```

The interesting thing is that function pointers were the slowest, followed by `u16` indexing and lastly `match` I had
before was the fastest 🤔

The branch miss rate is especially bad for direct function pointer calls, and IPC was worse for both "optimized"
versions, while `match` was sometimes reaching 4.0 IPC. Can't explain it better than that LLVM just can't optimize
multiple function calls nearly as well as a simple loop with a large `match` in it. And computed goto is not a thing in
Rust to avoid the function call cost, unfortunately.

So after spending a bunch of time on macros, I was back to the square one.

## Instruction fusion and unchecked memory access

The high IPC is a testament to carefully written code that CPU can predict efficiently, but high IPC doesn't mean those
instructions are doing something useful. I thought that since calling functions and instruction dispatch more generally
is relatively expensive in a basic interpreter, it'd be nice to reduce it.

The next thing I tried was instruction fusion. Essentially, detecting sequences of instructions and replacing them with
macro-fused instruction, which does the same thing but requires a single physical dispatch to the interpreter side,
amortizing dispatch cost.

To start, I noticed `ld` (64-bit load) and `sd` (64-bit store) instructions are quite common and often share the same
`rs1` register and have monotonically increasing/decreasing `rd` register and `imm` values in the same or opposite
direction. Granted, often those are used at the start/end of the function, but it still is a frequent pattern in a real
execution trace with double-digit percentage of runtime instruction. I'm talking about code patterns like this:

```asm
# Loads
ld s2,464(sp)
ld s3,456(sp)
ld s4,448(sp)
# Stores
sd a0,32(sp)
sd a1,40(sp)
sd a2,48(sp)
sd a3,56(sp)
```

Another observation is that often `rs1 = sp`, meaning a memory location relative to the stack pointer. If we know where
the stack is in memory and the whole memory layout is static (like it is in contracts), often those locations can be
statically determined to be in-bounds and bound checks can be skipped both for individual instructions and the whole
batches of instructions like above.

So I wrote an instruction optimizer that walks decoded instructions backwards, and for a sequence of matching
instructions it rewrites an earlier instruction with a fuse instruction of a "larger" size. This way it is possible to
process all instructions strictly sequentially, retain support for jumping at any instruction dynamically, while
"stepping over" fused instructions when executing instructions sequentially.

The code is quite tedious, so I'll just show load implementation as a representative instruction:

<details>
<summary>Load implementation</summary>

```rust
/// Optimize instructions into macro and unchecked instructions whenever possible.
///
/// `sp_range` is the range of stack pointer addresses.
/// `ro_memory_range` and `rw_memory_range` are valid memory addresses for reads and writes.
pub fn optimize(
    instructions: &mut [Self],
    sp_range: RangeInclusive<Reg::Type>,
    ro_memory_range: RangeInclusive<Reg::Type>,
    rw_memory_range: RangeInclusive<Reg::Type>,
) {
    for index in (0..instructions.len().saturating_sub(1)).rev() {
        // SAFETY: `index` and `index + 1` are always valid, in-bounds and distinct
        let [prev_instruction, &mut next_instruction] =
            unsafe { instructions.get_disjoint_unchecked_mut([index, index + 1]) };

        match *prev_instruction {
            Self::Ld { rd, rs1, imm } => {
                if rd.is_zero() {
                    // Writes to zero are no-op and fusion must ignore it
                    continue;
                }
                // Fusing `ld` instructions when possible
                match next_instruction {
                    Self::FusedLdSameDirection(fused_instruction) => {
                        if rs1 != fused_instruction.rs1 {
                            continue;
                        }
                        let Some(new_fuse_count) = fused_instruction.fuse_count.increment()
                        else {
                            continue;
                        };

                        let rd_offset = rd.offset();
                        let base_rd_offset = fused_instruction.rd_base.offset();
                        let imm_unsigned = i64::from(imm).cast_unsigned();
                        let imm_base_unsigned =
                            i64::from(fused_instruction.imm_base).cast_unsigned();
                        let step = size_of::<Reg::Type>() as u64;
                        let count = u8::from(fused_instruction.fuse_count);

                        // Extension from the low end:
                        // prev.rd == rd_base - 1
                        // prev.imm == imm_base - size_of::<Reg::Type>()
                        let low_rd_matches = rd_offset == base_rd_offset - 1;
                        let low_imm_matches =
                            imm_unsigned == imm_base_unsigned.wrapping_sub(step);

                        // Extension from the high end:
                        // prev.rd == rd_base + count
                        // prev.imm == imm_base + count * size_of::<Reg::Type>()
                        let high_rd_matches = rd_offset == base_rd_offset + count;
                        let high_imm_matches = imm_unsigned
                            == imm_base_unsigned.wrapping_add(u64::from(count) * step);

                        let (new_rd_base, new_imm_base) = if low_rd_matches && low_imm_matches {
                            // New instruction becomes the new base
                            (rd, imm)
                        } else if high_rd_matches && high_imm_matches {
                            // Base stays the same, just count increases
                            (fused_instruction.rd_base, fused_instruction.imm_base)
                        } else {
                            continue;
                        };

                        let unchecked = rs1.is_sp()
                            && sp_access_in_range(
                            new_imm_base,
                            u8::from(new_fuse_count),
                            &sp_range,
                            &ro_memory_range,
                        );

                        *prev_instruction = Self::FusedLdSameDirection(FusedLdSameDirection {
                            rd_base: new_rd_base,
                            fuse_count: new_fuse_count,
                            rs1,
                            imm_base: new_imm_base,
                            unchecked,
                        });
                    }
                    Self::FusedLdOppositeDirection(fused_instruction) => {
                        if rs1 != fused_instruction.rs1 {
                            continue;
                        }
                        let Some(new_fuse_count) = fused_instruction.fuse_count.increment()
                        else {
                            continue;
                        };

                        let rd_offset = rd.offset();
                        let base_rd_offset = fused_instruction.rd_base.offset();
                        let imm_unsigned = i64::from(imm).cast_unsigned();
                        let imm_base_unsigned =
                            i64::from(fused_instruction.imm_base).cast_unsigned();
                        let step = size_of::<Reg::Type>() as u64;
                        let count = u8::from(fused_instruction.fuse_count);

                        // Extension from the low end (rd goes down, imm goes up):
                        // prev.rd == rd_base - 1
                        // prev.imm == imm_base + count * size_of::<Reg::Type>()
                        let low_rd_matches = rd_offset == base_rd_offset - 1;
                        let low_imm_matches = imm_unsigned
                            == imm_base_unsigned.wrapping_add(u64::from(count) * step);

                        // Extension from the high end (rd goes up, imm goes down):
                        // prev.rd == rd_base + count
                        // prev.imm == imm_base - 8
                        let high_rd_matches = rd_offset == base_rd_offset + count;
                        let high_imm_matches =
                            imm_unsigned == imm_base_unsigned.wrapping_sub(step);

                        let (new_rd_base, new_imm_base) = if low_rd_matches && low_imm_matches {
                            // New lowest `rd_base`, `imm_base` unchanged
                            (rd, fused_instruction.imm_base)
                        } else if high_rd_matches && high_imm_matches {
                            // New lowest `imm_base`, `rd_base` unchanged
                            (fused_instruction.rd_base, imm)
                        } else {
                            continue;
                        };

                        let unchecked = rs1.is_sp()
                            && sp_access_in_range(
                            new_imm_base,
                            u8::from(new_fuse_count),
                            &sp_range,
                            &ro_memory_range,
                        );

                        *prev_instruction =
                            Self::FusedLdOppositeDirection(FusedLdOppositeDirection {
                                rd_base: new_rd_base,
                                fuse_count: new_fuse_count,
                                rs1,
                                imm_base: new_imm_base,
                                unchecked,
                            });
                    }
                    Self::Ld {
                        rd: next_rd,
                        rs1: next_rs1,
                        imm: next_imm,
                    } => {
                        if rs1 != next_rs1 {
                            continue;
                        }

                        let rd_offset = rd.offset();
                        let next_rd_offset = next_rd.offset();
                        let imm_unsigned = i64::from(imm).cast_unsigned();
                        let next_imm_unsigned = i64::from(next_imm).cast_unsigned();
                        let step = size_of::<Reg::Type>() as u64;

                        if !(rd_offset.abs_diff(next_rd_offset) == 1
                            && imm_unsigned.abs_diff(next_imm_unsigned) == step)
                        {
                            continue;
                        }

                        let rd_increasing = rd_offset < next_rd_offset;
                        let imm_increasing = imm_unsigned < next_imm_unsigned;

                        // Ensure base `imm` is always increasing, starting at the `imm_base`
                        let imm_base = if imm_increasing { imm } else { next_imm };
                        // Ensure `rd` is always increasing, starting at the `rd_base`
                        let rd_base = if rd_increasing { rd } else { next_rd };

                        let fuse_count = FuseCount::X2;
                        let unchecked = rs1.is_sp()
                            && sp_access_in_range(
                            imm_base,
                            u8::from(fuse_count),
                            &sp_range,
                            &ro_memory_range,
                        );

                        // Pick the correct direction of the fused instruction
                        *prev_instruction = if rd_increasing == imm_increasing {
                            Self::FusedLdSameDirection(FusedLdSameDirection {
                                rd_base,
                                fuse_count,
                                rs1,
                                imm_base,
                                unchecked,
                            })
                        } else {
                            Self::FusedLdOppositeDirection(FusedLdOppositeDirection {
                                rd_base,
                                fuse_count,
                                rs1,
                                imm_base,
                                unchecked,
                            })
                        };
                    }
                    _ => {
                        // ..
                    }
                }
            }
            Self::Sd { rs2, rs1, imm } => {
                // Very similar
            }
            _ => {
                // Ignore
            }
        }
    }
}
```

</details>

The big question is: did it work?

Well, kinda. Sometimes a bit faster, sometimes substantially slower, sometimes about the same. Overall, it wasn't a
clear win I was hoping for, which left me puzzled.

## Zero register check improves performance

Zero register in RISC-V is special: it is always read as `0` and any write to it is no-op. So the implementation of the
register file I had looks like this:

```rust
/// A set of RISC-V GPRs (General Purpose Registers)
#[derive(Debug, Clone, Copy)]
#[repr(align(16))]
pub struct Registers<Reg>
where
    Reg: Register,
{
    regs: [Reg::Type; Reg::N],
}

const impl<Reg> Registers<Reg>
where
    Reg: [ const ] Register,
{
    /// Read register value
    #[inline(always)]
    pub fn read(&self, reg: Reg) -> Reg::Type {
        if reg.is_zero() {
            // Always zero
            return Reg::Type::default();
        }

        // SAFETY: register offset is always within bounds
        *unsafe { self.regs.get_unchecked(usize::from(reg.offset())) }
    }

    /// Write register value
    #[inline(always)]
    pub fn write(&mut self, reg: Reg, value: Reg::Type) {
        if reg.is_zero() {
            // Writes are ignored
            return;
        }

        // SAFETY: register offset is always within bounds
        *unsafe { self.regs.get_unchecked_mut(usize::from(reg.offset())) } = value;
    }
}
```

Note two `reg.is_zero()` checks. They seem redundant, I should be able to remove one or the other, and the result will
still be correct since either writes will still be ignored (registers are initialized to zero) or reads will ignore
whatever was written at the offset of zero register. At the same time, fewer branches should mean higher performance.

However, as I removed either of those, the performance was measurably and consistently A LOT worse. Hm...

## More profiling

I went back to profiling, this time with Claude's help since I can't make sense of a lot of the metrics quite as
productively.

IPC was hitting 4.1, 0.6% data cache miss rate, 99.96% op cache hit rate:

```
     3.003641903         21 958 894      ex_ret_brn_misp                  #      5,5 %  bad_speculation_mispredicts
     3.003641903                                                          #      0,0 %  bad_speculation_pipeline_restarts  (19,97%)
     3.003641903     20 284 275 971      de_src_op_disp.all               #      5,5 %  bad_speculation          (19,97%)
     3.003641903             51 318      resyncs_or_nc_redirects                                                 (19,97%)
     3.003641903      4 895 089 444      ls_not_halted_cyc                                                       (19,97%)
     3.003641903     18 657 737 631      ex_ret_ops                       #     63,5 %  retiring                 (19,97%)
     3.003641903        681 847 357      ex_no_retire.load_not_complete   #      4,4 %  backend_bound_cpu      
     3.003641903                                                          #      7,8 %  backend_bound_memory     (20,10%)
     3.003641903      3 602 363 546      de_no_dispatch_per_slot.backend_stalls #     12,3 %  backend_bound            (20,10%)
     3.003641903      1 068 036 530      ex_no_retire.not_complete                                               (20,10%)
     3.003641903      4 894 584 247      ls_not_halted_cyc                                                       (20,10%)
     3.003641903          8 854 312      ex_ret_ucode_ops                 #     63,4 %  retiring_fastpath      
     3.003641903                                                          #      0,0 %  retiring_microcode       (19,97%)
     3.003641903      4 895 098 597      ls_not_halted_cyc                                                       (19,97%)
     3.003641903     18 620 594 832      ex_ret_ops                                                              (19,97%)
     3.003641903      5 460 578 458      de_no_dispatch_per_slot.no_ops_from_frontend #     18,6 %  frontend_bound         
     3.003641903                                                          #      7,1 %  frontend_bound_bandwidth  (19,98%)
     3.003641903        563 718 873      cpu/de_no_dispatch_per_slot.no_ops_from_frontend,cmask=0x6/ #     11,5 %  frontend_bound_latency   (19,98%)
     3.003641903      4 894 729 709      ls_not_halted_cyc                                                       (19,98%)
     3.003641903         79 739 703      de_no_dispatch_per_slot.smt_contention #      0,3 %  smt_contention           (39,95%)
     3.003641903      4 894 939 755      ls_not_halted_cyc                                                       (39,95%)
```

We then discovered a suspiciously high `ls_bad_status2.stli_other`:

```
     3.003264296      4 878 085 756      cycles                                                                  (83,62%)
     3.003264296      3 047 870 159      ls_dispatch.ld_dispatch                                                 (83,44%)
     3.003264296            566 760      l1-dcache-load-misses                                                   (83,22%)
     3.003264296      3 312 510 978      l1-dcache-loads                                                         (83,22%)
     3.003264296         46 457 934      ls_stlf                                                                 (83,22%)
     3.003264296        434 670 576      ls_bad_status2.stli_other                                               (83,28%)
```

This is non-forwardable store-to-load conflicts: a store and a subsequent load overlap in address space but the CPU
can't forward the value directly, so the load has to wait for the store to commit and then re-read from cache. This is
expensive (~15 cycles penalty each).

Claude suggested this is the most likely performance constraint right now, likely caused by the register file, which is
hit on every single instruction, often multiple times. This also explains why removing a seemingly harmless
`reg.is_zero()` check was a bad trade of branch for extra memory access (zero register usage is ubiquitous in RISC-V).

## Memory is slow

Turns out, accessing registers is almost free and accessing memory for each register access is very expensive. What do
we do about that then?

Well, the obvious solution would be to not access memory in the first place, but how? Essentially, we need to convince
LLVM during code generation to use native registers for at least some of the most popular RISC-V registers. Right now
with `[Reg::Type; Reg::N]` used for the register file, this is prevented fundamentally due to aliasing, but even if it
wasn't the case, the fact that instruction execution takes the whole state as an argument, even if it wasn't an array,
accessing registers would still be a memory operation anyway:

```rust
/// Trait for executable instructions
pub trait ExecutableInstruction<State, CustomError = CustomErrorPlaceholder>
where
    Self: Instruction,
{
    // ..

    /// Execute instruction
    fn execute(
        self,
        state: &mut State,
    ) -> Result<ControlFlow<()>, ExecutionError<Address<Self>, CustomError>>;
}

```

The solution is to store the register file in a local variable at the beginning of the execution loop with at least
popular registers being its separate fields and carefully design and inline everything such that the register file is
never aliased as a whole. If the conditions are right, LLVM should be able to treat popular registers as, effectively,
standalone variables, and if we can convince it (with explicit hints or PGO) that those "variables" are really hot,
it'll generate code that uses native registers for them.

## Conclusion (so far)

That is basically where I am right now in terms of performance optimizations. I believe there is a substantial uplift to
be unlocked soon while keeping the design modular and easy to use, but it'll require a small architectural change to
make it happen.

This discovery with expensive registers is also what led me to implement support for some compressed instructions, which
often encapsulate RISC-V idioms more compactly and can avoid even trying to access zero register, for example. I have
not added them to the benchmark to estimate the practical performance implications, though.

## Upcoming plans

That was a lot of words, I hope you found it at least somewhat useful for whatever you work on today or may work in the
future. If you have some lessons learned to share that might help here, please let me know on [Zulip].

[Zulip]: https://abundance.zulipchat.com/

In the nearest future I want to tackle that architectural change and see if I can promote host RISC-V registers to
native registers and see what it does to performance. If it helps, I'll revisit if instruction fusion is still worth
doing after that. It is quite fun working on this stuff, and the fact that it is generic and potentially usable by
others only adds to the value of this work in my eyes.

I also plan to add some interpreter usage examples for others to be able to start with. There are quite a few in the
repo already, but having something simple and standalone would be great to have.

I'll write again some time soon with more updates, see you then!
