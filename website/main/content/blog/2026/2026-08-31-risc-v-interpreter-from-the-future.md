---
title: RISC-V interpreter from the future
date: 2026-08-31
draft: false
description: High-performance RISC-V interpreter with a lot of compelling features using nightly Rust
tags: [ announcement ]
authors: [ nazar-pc ]
---

There were a lot of changes to the RISC-V interpreter I announced [a while back], and I think it is now in a state that
is usable for more people.

A short list of features to get you interested: fully modular and generic, panic-free, `no_std` (and zero allocations),
runs at compile time (`const fn`), implements RISC-V specification strictly (supposed to be suitable for blockchain
purposes), passes [RISC-V Architectural Certification Tests] and is pretty fast while doing all that.

[a while back]: ../2026-03-14-first-crates-on-crates.io
[RISC-V Architectural Certification Tests]: https://github.com/riscv/riscv-arch-test

The cost of all this? It is in the title of the post: ~30 nightly Rust features from advanced const generics to
guaranteed tail calls, to achieve some really nice things impossible with stable Rust today. I hope most of these will
be stabilized in the not-so-distant future.

<!--more-->

The details below correspond to 0.2 releases of [ab-riscv-interpreter] and [ab-riscv-primitives] crates.

[ab-riscv-interpreter]: https://crates.io/crates/ab-riscv-interpreter
[ab-riscv-primitives]: https://crates.io/crates/ab-riscv-primitives

---

## Fully modular and generic

RISC-V specification is modular, it consists of a few variants of base ISA and a lot of extensions. The implementation
of the interpreter is done the same way: base ISA and each extension are implemented separately and can be composed in
any way allowed by the specification. Not only that, things like memory, register file and even the type of the general
purpose register are all generic. Extension-specific environment details are also generic, so things like additional
registers (floats, vectors, etc.) can be implemented modularly too.

### Instruction definition and decoding

Instruction definition and decoding are actually implemented in a separate crate, so if you just need a spec-compliant
decoder and not the interpreter, you can totally use it separately.

Here is an example of instruction decoding for a simple extension:

```rust
/// RISC-V Zicond instruction (Integer Conditional Operations)
#[instruction]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
pub enum ZicondInstruction<Reg> {
    /// `czero.eqz rd, rs1, rs2` - move zero to `rd` if `rs2 == 0`, else move `rs1`
    CzeroEqz { rd: Reg, rs1: Reg, rs2: Reg },
    /// `czero.nez rd, rs1, rs2` - move zero to `rd` if `rs2 != 0`, else move `rs1`
    CzeroNez { rd: Reg, rs1: Reg, rs2: Reg },
}

#[instruction]
const impl<Reg> Instruction for ZicondInstruction<Reg>
where
    Reg: [const] Register,
{
    type Reg = Reg;

    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn try_decode(instruction: u32) -> Option<Self> {
        let opcode = (instruction & 0b111_1111) as u8;
        let rd_bits = ((instruction >> 7) & 0x1f) as u8;
        let funct3 = ((instruction >> 12) & 0b111) as u8;
        let rs1_bits = ((instruction >> 15) & 0x1f) as u8;
        let rs2_bits = ((instruction >> 20) & 0x1f) as u8;
        let funct7 = ((instruction >> 25) & 0x7f) as u8;

        // Both Zicond instructions share opcode=0x33 (OP) and funct7=0x07
        match (opcode, funct7) {
            (0b011_0011, 0b000_0111) => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                match funct3 {
                    0b101 => Some(Self::CzeroEqz { rd, rs1, rs2 }),
                    0b111 => Some(Self::CzeroNez { rd, rs1, rs2 }),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    #[inline(always)]
    fn alignment() -> u8 {
        align_of::<u32>() as u8
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}
```

As you can see, the implementation is fairly basic and is more or less what you'd expect after reading the
specification. For composability purposes there are some minor requirements like only instantiating enum with `Self::`
or not using `return`, so it is easier to process later.

[#\[instruction\]] on the enum definition also supports a bunch of options that allow specifying dependencies:

[#\[instruction\]]: https://docs.rs/ab-riscv-macros/0.1.1/ab_riscv_macros/attr.instruction.html

```rust
#[instruction(inherit = [Rv32ZaamoInstruction])]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
#[rustfmt::skip]
pub enum Rv32ZabhaInstruction<Reg> {
    AmoswapB { rd: Reg, rs1: Reg, rs2: Reg, aq: bool, rl: bool },
    // ...
    AmomaxuH { rd: Reg, rs1: Reg, rs2: Reg, aq: bool, rl: bool },
    /// Compare-and-swap byte. Only present when `Zacas` is also implemented.
    #[instruction(if = [Rv32ZacasInstruction])]
    AmocasB { rd: Reg, rs1: Reg, rs2: Reg, aq: bool, rl: bool },
    /// Compare-and-swap halfword. Only present when `Zacas` is also implemented.
    #[instruction(if = [Rv32ZacasInstruction])]
    AmocasH { rd: Reg, rs1: Reg, rs2: Reg, aq: bool, rl: bool },
}
```

As you can see, both are simple inheritance/dependency, and instructions predicated on the presence of another
instruction are expressible. The only thing not implemented is instruction conflicts, but that will come once the first
extension of such kind is implemented (likely Zcd).

Combined decoding is basically a concatenation of individual `try_decode()` functions.

There are a lot of new types there to make sure invalid instructions do not even decode, and additional constraints can
be specified on the register type to maintain correct invariants:

```rust
#[instruction]
const impl<Reg> Instruction for Rv32ZcmpOnlyInstruction<Reg>
where
    Reg: [const] ZcmpRegister<Type=u32>,
{
    type Reg = Reg;
// ...
```

You can also exclude instructions that you don't want to support, so they do not decode. For example, this excludes
`ecall` instruction, while keeping everything else as is:

```rust
#[instruction(
    ignore = [Ecall],
    inherit = [
        Rv64ZcaInstruction,
        Rv64ZcbInstruction,
        Rv64ZcmpInstruction,
        Rv64Instruction,
        Rv64MInstruction,
        Rv64BInstruction,
        Rv64ZbcInstruction,
        Rv64ZknInstruction,
        ZicondInstruction,
    ],
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractInstruction<Reg = ContractRegister> {}
```

There is also a way to reorder instructions, see macro definition for details.

### Stateful macros

You might be surprised that dependencies are expressed that way and then decoding bodies are concatenated. This requires
stateful macros that share data between invocations, and Rust doesn't support that. At least not directly.

The solution is to have a build script that scans all the files and generates implementations, while a proc macro simply
replaces the original code with [include!()]:

```rust
use ab_riscv_macros::process_instruction_macros;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    process_instruction_macros()?;

    Ok(())
}
```

[include!()]: https://doc.rust-lang.org/core/macro.include.html

[process_instruction_macros ()] maintains information about all instructions in the crate and pulls [crate metadata]
from crate dependencies, so it is able to resolve dependencies between instructions and generate necessary
implementations:

[process_instruction_macros ()]: https://docs.rs/ab-riscv-macros/0.1.1/ab_riscv_macros/fn.process_instruction_macros.html
[crate metadata]: https://doc.rust-lang.org/cargo/reference/build-scripts.html#the-links-manifest-key

One complication is that, as you can see, nightly features like const trait syntax are used, which `syn` doesn't support
yet. The solution for that is [a small hack] that converts nightly syntax into something that is valid stable Rust
syntax and then converts it back after all the processing.

[a small hack]: https://docs.rs/ab-riscv-macros-common/0.1.0/src/ab_riscv_macros_common/code_utils.rs.html

### Instruction execution

Instruction execution also needs to follow certain requirements and be annotated with [#\[instruction_execution\]]
macro, but otherwise looks about what you'd expect too:

[#\[instruction_execution\]]: https://docs.rs/ab-riscv-macros/0.1.1/ab_riscv_macros/attr.instruction_execution.html

```rust
#[instruction_execution]
const impl<Reg, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
for Rv32ZbsInstruction<Reg>
where
    Reg: [const] Register<Type=u32>,
    Regs: [const] RegisterFile<Reg>,
{
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn execute(
        self,
        Rs1Rs2OperandValues {
            rs1_value,
            rs2_value,
        }: Rs1Rs2OperandValues<<Self::Reg as Register>::Type>,
        _regs: &mut Regs,
        _env: &mut Env,
        _memory: &mut Memory,
        _program_counter: &mut PC,
    ) -> ExecutionResult<Self::Reg> {
        match self {
            Self::Bset { rd, rs1: _, rs2: _ } => {
                let index = rs2_value & 0x1f;
                let result = rs1_value | (1u32 << index);
                ExecutionResult::Continue { rd, value: result }
            }
            Self::Bseti { rd, rs1: _, shamt } => {
                let index = shamt;
                let result = rs1_value | (1u32 << index);
                ExecutionResult::Continue { rd, value: result }
            }
            Self::Bclr { rd, rs1: _, rs2: _ } => {
                let index = rs2_value & 0x1f;
                let result = rs1_value & !(1u32 << index);
                ExecutionResult::Continue { rd, value: result }
            }
            Self::Bclri { rd, rs1: _, shamt } => {
                let index = shamt;
                let result = rs1_value & !(1u32 << index);
                ExecutionResult::Continue { rd, value: result }
            }
            Self::Binv { rd, rs1: _, rs2: _ } => {
                let index = rs2_value & 0x1f;
                let result = rs1_value ^ (1u32 << index);
                ExecutionResult::Continue { rd, value: result }
            }
            Self::Binvi { rd, rs1: _, shamt } => {
                let index = shamt;
                let result = rs1_value ^ (1u32 << index);
                ExecutionResult::Continue { rd, value: result }
            }
            Self::Bext { rd, rs1: _, rs2: _ } => {
                let index = rs2_value & 0x1f;
                let result = (rs1_value >> index) & 1;
                ExecutionResult::Continue { rd, value: result }
            }
            Self::Bexti { rd, rs1: _, shamt } => {
                let index = shamt;
                let result = (rs1_value >> index) & 1;
                ExecutionResult::Continue { rd, value: result }
            }
        }
    }
}
```

The API has some good reasons to look the way it does, but what "macro" does under the hood is even more interesting!

`fn execute()` is written with a single `match` such that it is easy to parse. After that a bunch of code is generated
(~1900 lines for the above implementation).

First, for each instruction a standalone function is extracted that looks like this:

```rust
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
#[inline(always)]
const fn execute_rv32_zbs_instruction_bset<Reg, Regs, Env, Memory, PC>(
    rd: Reg,
    rs1_value: <<Rv32ZbsInstruction<Reg> as Instruction>::Reg as Register>::Type,
    rs2_value: <<Rv32ZbsInstruction<Reg> as Instruction>::Reg as Register>::Type,
    regs: &mut Regs,
    env: &mut Env,
    memory: &mut Memory,
    program_counter: &mut PC,
) -> ExecutionResult<<Rv32ZbsInstruction<Reg> as Instruction>::Reg>
where
    Reg: [const]  Register<Type=u32>,
    Regs: [const]  RegisterFile<Reg>,
{
    {
        let _ = rd;
        let _ = rs1_value;
        let _ = rs2_value;
        let _ = regs;
        let _ = env;
        let _ = memory;
        let _ = program_counter;
    }
    {
        let index = rs2_value & 0x1f;
        let result = rs1_value | (1u32 << index);
        ExecutionResult::Continue {
            rd,
            value: result,
        }
    }
}
```

After that, original `match` arms are replaced with calls to this function. Eventually, all `match` arms from all
dependencies are combined into a single large `match` that is used for execution. This produces a reasonably fast and
compact implementation, but it is far from peak performance. For peak performance indirect threading implementation is
also generated with platform-specific ABI that overall looks something like this (on x86-64):

```rust
// ...

impl<Reg, Regs, Env, Memory, PC> ThreadedExecutableInstruction<Regs, Env, Memory, PC>
for Rv32ZbsInstruction<Reg>
where
    Reg: Register<Type=u32>,
    Regs: RegisterFile<Reg>,
    PC: InstructionFetcher<Rv32ZbsInstruction<Reg>, Memory>,
{
    #[inline(always)]
    fn execute_threaded(
        instruction_fetcher: PC,
        regs: &mut Regs,
        env: Env,
        memory: &mut Memory,
    ) -> ThreadedExecutionResult<Rv32ZbsInstruction<Reg>> {
        if !OpaqueThreadedExecutionResult::<
            Rv32ZbsInstruction<Reg>,
        >::platform_supported() {
            ::core::hint::cold_path();
            return ThreadedExecutionResult::failed(
                instruction_fetcher.get_pc(),
                ExecutionError::UnsupportedPlatform,
            );
        }
        unsafe {
            execute_rv32_zbs_instruction_threaded::<
                Reg,
                Regs,
                Env,
                Memory,
                PC,
            >(instruction_fetcher, regs, env, memory)
        }
    }
}
```

<details>
<summary>Lower-level details</summary>

```rust
// ...

#[rustc_align(64)]
#[cfg_attr(any(not(miri), target_feature = "avx"), target_feature(enable = "avx"))]
unsafe extern "sysv64" fn execute_rv32_zbs_instruction_bset_threaded<
    Reg,
    Regs,
    Env,
    Memory,
    PC,
>(
    instruction: Rv32ZbsInstruction<Reg>,
    mut instruction_fetcher: PC,
    regs: &mut Regs,
    mut env: Env,
    memory: &mut Memory,
) -> OpaqueThreadedExecutionResult<Rv32ZbsInstruction<Reg>>
where
    Reg: Register<Type=u32>,
    Regs: RegisterFile<Reg>,
    PC: InstructionFetcher<Rv32ZbsInstruction<Reg>, Memory>,
{
    let Rs1Rs2Operands { rs1, rs2 } = instruction.get_rs1_rs2_operands();
    let rs1_value = regs.read(rs1);
    let rs2_value = regs.read(rs2);
    let Rv32ZbsInstruction::Bset { rd, rs1: _, rs2: _ } = instruction else {
        unsafe {
            ::core::hint::unreachable_unchecked();
        }
    };
    unsafe {
        instruction_fetcher.advance(Instruction::size(&instruction));
    }
    let execution_result = execute_rv32_zbs_instruction_bset::<
        Reg,
        Regs,
        Env,
        Memory,
        PC,
    >(rd, rs1_value, rs2_value, regs, &mut env, memory, &mut instruction_fetcher);
    let control_flow = match execution_result {
        ExecutionResult::Continue { rd, value } => {
            regs.write(rd, value);
            Ok(::core::ops::ControlFlow::Continue(()))
        }
        ExecutionResult::ContinueNoWrite => Ok(::core::ops::ControlFlow::Continue(())),
        ExecutionResult::Branch { offset } => {
            if unsafe {
                instruction_fetcher
                    .try_set_pc_relative(Instruction::size(&instruction), offset)
            } {
                Ok(::core::ops::ControlFlow::Continue(()))
            } else {
                unsafe {
                    become
                    rv32_zbs_instruction_threaded_branch_failed::<
                        Reg,
                        Regs,
                        Env,
                        Memory,
                        PC,
                    >(instruction, instruction_fetcher, regs, env, memory)
                }
            }
        }
        ExecutionResult::Jump { target } => instruction_fetcher.set_pc(memory, target),
        ExecutionResult::Break => {
            ::core::hint::cold_path();
            return unsafe {
                OpaqueThreadedExecutionResult::new(
                    ThreadedExecutionResult::stopped(instruction_fetcher.get_pc()),
                )
            };
        }
        ExecutionResult::Err(error) => {
            ::core::hint::cold_path();
            return unsafe {
                OpaqueThreadedExecutionResult::new(
                    ThreadedExecutionResult::failed(instruction_fetcher.get_pc(), error),
                )
            };
        }
    };
    match control_flow {
        Ok(::core::ops::ControlFlow::Continue(())) => {}
        Ok(::core::ops::ControlFlow::Break(())) => {
            ::core::hint::cold_path();
            return unsafe {
                OpaqueThreadedExecutionResult::new(
                    ThreadedExecutionResult::stopped(instruction_fetcher.get_pc()),
                )
            };
        }
        Err(error) => {
            ::core::hint::cold_path();
            return unsafe {
                OpaqueThreadedExecutionResult::new(
                    ThreadedExecutionResult::failed(instruction_fetcher.get_pc(), error),
                )
            };
        }
    }
    let (instruction, handler) = match dispatch_rv32_zbs_instruction::<
        Reg,
        Regs,
        Env,
        Memory,
        PC,
    >(&mut instruction_fetcher, memory) {
        Rv32ZbsInstructionThreadedDispatchResult::Next { instruction, handler } => {
            (instruction, handler)
        }
        Rv32ZbsInstructionThreadedDispatchResult::Break => {
            ::core::hint::cold_path();
            return unsafe {
                OpaqueThreadedExecutionResult::new(
                    ThreadedExecutionResult::stopped(instruction_fetcher.get_pc()),
                )
            };
        }
        Rv32ZbsInstructionThreadedDispatchResult::Err(error) => {
            ::core::hint::cold_path();
            return unsafe {
                OpaqueThreadedExecutionResult::new(
                    ThreadedExecutionResult::failed(instruction_fetcher.get_pc(), error),
                )
            };
        }
    };
    unsafe {
        become
        handler(instruction, instruction_fetcher, regs, env, memory)
    }
}

// ...

#[inline(always)]
fn dispatch_rv32_zbs_instruction<Reg, Regs, Env, Memory, PC>(
    instruction_fetcher: &mut PC,
    memory: &Memory,
) -> Rv32ZbsInstructionThreadedDispatchResult<
    Rv32ZbsInstruction<Reg>,
    unsafe extern "sysv64" fn(
        Rv32ZbsInstruction<Reg>,
        PC,
        &mut Regs,
        Env,
        &mut Memory,
    ) -> OpaqueThreadedExecutionResult<Rv32ZbsInstruction<Reg>>,
>
where
    Reg: Register<Type=u32>,
    Regs: RegisterFile<Reg>,
    PC: InstructionFetcher<Rv32ZbsInstruction<Reg>, Memory>,
{
    let instruction = loop {
        match instruction_fetcher.peek_instruction(memory) {
            FetchInstructionResult::Instruction(instruction) => {
                break instruction;
            }
            FetchInstructionResult::Continue => {
                ::core::hint::cold_path();
            }
            FetchInstructionResult::Break => {
                ::core::hint::cold_path();
                return Rv32ZbsInstructionThreadedDispatchResult::Break;
            }
            FetchInstructionResult::Err(error) => {
                ::core::hint::cold_path();
                return Rv32ZbsInstructionThreadedDispatchResult::Err(error);
            }
        }
    };
    let handler = match instruction {
        Rv32ZbsInstruction::Bset { .. } => {
            execute_rv32_zbs_instruction_bset_threaded::<Reg, Regs, Env, Memory, PC>
        }
        // ...
    };
    Rv32ZbsInstructionThreadedDispatchResult::Next {
        instruction,
        handler,
    }
}

#[inline]
#[cfg_attr(any(not(miri), target_feature = "avx"), target_feature(enable = "avx"))]
unsafe fn execute_rv32_zbs_instruction_threaded<Reg, Regs, Env, Memory, PC>(
    mut instruction_fetcher: PC,
    regs: &mut Regs,
    env: Env,
    memory: &mut Memory,
) -> ThreadedExecutionResult<Rv32ZbsInstruction<Reg>>
where
    Reg: Register<Type=u32>,
    Regs: RegisterFile<Reg>,
    PC: InstructionFetcher<Rv32ZbsInstruction<Reg>, Memory>,
{
    let (instruction, handler) = match dispatch_rv32_zbs_instruction::<
        Reg,
        Regs,
        Env,
        Memory,
        PC,
    >(&mut instruction_fetcher, memory) {
        Rv32ZbsInstructionThreadedDispatchResult::Next { instruction, handler } => {
            (instruction, handler)
        }
        Rv32ZbsInstructionThreadedDispatchResult::Break => {
            ::core::hint::cold_path();
            return ThreadedExecutionResult::stopped(instruction_fetcher.get_pc());
        }
        Rv32ZbsInstructionThreadedDispatchResult::Err(error) => {
            ::core::hint::cold_path();
            return ThreadedExecutionResult::failed(instruction_fetcher.get_pc(), error);
        }
    };
    let outcome = unsafe {
        handler(instruction, instruction_fetcher, regs, env, memory)
    };
    outcome.into_result()
}
```

</details>

That is a lot of boilerplate generated to help the compiler to generate efficient implementation. One interesting trick
used there is returning more than 16 bytes from a threaded function using SIMD register to keep all 6 registers of
`sysv64` ABI available for input arguments.

## Panic-free implementation

You may have noticed `no-panic` feature that adds extra attributes. It comes originally from the [no-panic] crate. The
idea is essentially to install a drop guard that calls a non-existing function via FFI and carefully remove it without
dropping after execution of the code. If no panics occurred between guard installation and removal, the compiler will
see that and remove the drop guard completely. If not, you'll get a not very detailed linker error that fails to find a
non-existing FFI function.

[no-panic]: https://crates.io/crates/no-panic

This is a clever hack, but it works. Unless you use `const fn` or even worse const traits. To deal with that, I created
a fork [no-panic-const] crate that adds support for both, and that is what you see in the examples above. Macros are
aware of this feature and preserve it in generated code.

[no-panic-const]: https://crates.io/crates/no-panic-const

In the end, you get a compile-time guarantee that panics are physically absent in the code, which is great for both
performance and general robustness. Avoiding allocations helps here too.

## `no_std`

The interpreter doesn't use the standard library even optionally, it only has `alloc` feature for blanket
implementations on boxed impls and doesn't allocate anything outside of test. You can run it anywhere, including bare
metal. However, even there you'll get feature detection (if possible) and hardware acceleration for things like RV64 AES
extensions on x86-64/aarch64 (implementation takes advantage of platform-specific intrinsics there).

## `const fn`

All base ISA variants (RV32I, RV32E, RV64I and RV64E) as well as all implemented extensions except vectors are usable at
compile time. It is useful for some things like instruction decoding (you can embed pre-decoded binary in a static or
constant) and probably less useful for execution as such, yet it was an interesting exercise.

You can totally run something like RV64IMAC as your application compiles and include only the result of the execution in
the final binary. Don't think you'll need that very often, but it is pretty cool, right?

One complication came when implementing it, which was platform-specific intrinsics. Only some on x86-64 are `const fn`,
everything else is not. Giving up intrinsics and other optimizations seemed quite unfortunate for such a feature. There
is [const_eval_select] compiler intrinsic that allows calling a different function depending on whether it is called at
compile time or runtime. It isn't particularly ergonomic to use and is easy to trigger ICE with, so I had a proposal for
[`const fn` specialization], which I ended up implementing as a fairly pleasant proc macro crate
[const-fn-specialization]. If you have a similar use case, you might find it useful too.

[const_eval_select]: https://doc.rust-lang.org/nightly/core/intrinsics/fn.const_eval_select.html

[`const fn` specialization]: https://internals.rust-lang.org/t/const-fn-specialization/24488?u=nazar-pc

[const-fn-specialization]: https://crates.io/crates/const-fn-specialization

With it, it is possible to write the same function twice: one version for `const fn` and another for regular execution.
Here is a good demonstration of what I mean:

```rust
#[const_fn_specialization]
pub fn orc_b(src: u32) -> u32 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(not(miri), target_arch = "riscv32", target_feature = "zbb") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe { core::arch::riscv32::orc_b(src as usize) as u32 }
        }
        _ => orc_b_generic(src),
    }
}

#[const_fn_specialization]
pub const fn orc_b(src: u32) -> u32 {
    orc_b_generic(src)
}

const fn orc_b_generic(src: u32) -> u32 {
    let bytes = src.to_le_bytes();

    u32::from_le_bytes([
        if bytes[0] != 0 { 0xFF } else { 0 },
        if bytes[1] != 0 { 0xFF } else { 0 },
        if bytes[2] != 0 { 0xFF } else { 0 },
        if bytes[3] != 0 { 0xFF } else { 0 },
    ])
}
```

## What is supported?

If that looks interesting, here is what is supported today.

Base ISA: RV32I, RV32E, RV64I and RV64E.

I and E variants are mostly the same, only register generic is what's different
between them. BTW, you can use your own custom register type, for example, you can exclude `gp` and `tp` registers if
your single-threaded binary is not expected to have them at compile time anyway, so instructions trying to use them fail
to decode at all.

Extensions: A, M, B, Zaamo, Zabha, Zacas, Zalasr, Zalrsc, Zawrs, Zba, Zbb, Zbc, Zbkb, Zbkc, Zbkx, Zbs, Zca, Zcb, Zcmp,
Zicond, Zicsr, Zifencei, Zkn, Zknd, Zkne, Zknh, Zkr, Zvbb, Zvbc, Zve32x, Zve64x, Zvkb, Ssstrict. Any valid element and
vector length is supported too, in a `const` generic way (invalid permutations do not compile).

Yeah, that is a lot. Each is supported for both RV32 and RV64. Almost anything you might want in unprivileged integer
ISA you already have. Floats are the biggest missing piece that non-blockchain use cases might want, and then a bunch of
privileged stuff if you want to run Linux under it for whatever reason (I do not, not yet at least).

`Ssstrict` in particular means the implementation strictly decodes and interprets instructions and rejects things like
reserved encodings, so only well-defined deterministic behavior is allowed.

Moreover, extensions are either generic over GPR completely or only require a specific register width. This means not
only proper RISC-V code can be executed, but also any RISC-V-like ISA, for example, [PolkaVM].

[PolkaVM]: https://github.com/paritytech/polkavm

## ACT4

[RISC-V Architectural Certification Tests] (ACTs) are used on top of various unit tests to ensure implementation follows
the spec correctly. This was especially useful for the massive vector extension implementation. Some extensions do not
have official certification tests yet (Zalasr and Zcmp), but their implementation is straightforward enough as is.

## Performance

It is an interpreter, okay? Don't expect miracles. CoreMark score for compact optimized `match` loop implementation
reaches ~**1600** and indirect threading reaches ~**2950** on Zen4 CPU. Direct threading could be 10%+ faster than that
still, according to experiments, but no such code is automatically generated _today_.

BLAKE3 hashing and ed25519 signature verification run at ~2-5% of AVX512-optimized native throughput without vector
extension usage. Should be possible to go higher with vectors and especially if the libraries gain vector implementation
optimized for RISC-V rather than whatever auto-vectorizer manages to come up with.

## What is next?

There are a lot of things that could be done here. At some point the state used for code generation will become a public
API, allowing for some more creative uses. For example, it would be possible to generate direct threading implementation
as effortlessly (for the user) as the two current versions.

I also want to experiment with generalizing JIT variant of the execution. Probably Cranelift-based. It'd be really cool
to be able to express the logic of individual extensions in a similar way to the current interpreter and then generate a
custom JIT implementation for the permutation of extensions you actually want.

Some abstraction over CSRs would probably be nice too, and adding floats of different sizes will probably make it more
useful for a wider audience.

Abstraction for instruction fusion would be cool to have as well, this is where the next performance boost is expected
to be gained.

Decoder is a separate crate and can print instructions like disassembler output, but it can't parse strings back into
instructions, which I think would be kind of cool to support as well.

Also, I should probably add smaller and simpler examples in addition to the ones that can be found in the repository
so far.

So many ideas and not so much time to write/review it all...

## Conclusion

I hope that if you're shopping for a RISC-V interpreter in Rust, you'll give it a try. It is reasonably easy to use,
**very** flexible, and you can write custom instructions for it fairly easily.

If something doesn't work or appears to be missing, please do let me know [on Zulip] and I'll try to help.

[on Zulip]: https://abundance.zulipchat.com/

And that is enough words from me for now, see you next time!
