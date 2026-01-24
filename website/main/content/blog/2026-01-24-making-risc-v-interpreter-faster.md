---
title: Making RISC-V interpreter faster
date: 2026-01-24
draft: false
description: Exploration of current limitations of RISC-V interpreter and ideas for making it faster
tags: [ status-update ]
authors: [ nazar-pc ]
---

In the [last update] I introduced a basic RISC-V interpreter, but its performance was underwhelming, which was to be
expected, but still was something that I'd like to improve. So since the last update I have implemented infrastructure
for measuring performance, did a bunch of refactorings to hopefully make my work reusable by other projects in the
future, and even implemented some performance improvements with a solid idea of what to explore next.

In the process of doing it I ended up parsing and generating Rust code in *both* `build.rs` and procedural macros,
something I have never thought I'd end up doing, but let's start from the beginning.

[last update]: ../2025-12-29-contracts-cli-and-risc-v-interpreter

<!--more-->

## Baseline performance

As mentioned last time, there was a basic CLI for building a contract. It was possible to run it, but there was no
actual test that runs anything from a contract, and there was no nice programmatic way to even do that.

The first step was to extract contract building logic from CLI into a library in [PR 497], which with some follow-up
fixes in [PR 499] and [PR 509] allowed me to implement building, testing, and even benchmarking a basic contract
in [PR 510]. It is a contract that does BLAKE3 hashing and Ed25519 signature verification.

[PR 497]: https://github.com/nazar-pc/abundance/pull/497

[PR 499]: https://github.com/nazar-pc/abundance/pull/499

[PR 509]: https://github.com/nazar-pc/abundance/pull/509

[PR 510]: https://github.com/nazar-pc/abundance/pull/510

Why those? I expect them to be used often and, as a reminder, I plan to not have custom domain-specific
precompiles/instructions, so it is important for generic and popular algorithms to perform well, which it clearly didn't
at first:

```
file/parse-only         time:   [115.34 µs 117.01 µs 120.17 µs]
                        thrpt:  [8.3215 Kelem/s 8.5463 Kelem/s 8.6702 Kelem/s]
file/parse-with-methods time:   [109.65 µs 109.84 µs 109.99 µs]
                        thrpt:  [9.0920 Kelem/s 9.1046 Kelem/s 9.1198 Kelem/s]
file/iterate-methods    time:   [78.396 ns 78.753 ns 79.699 ns]
                        thrpt:  [12.547 Melem/s 12.698 Melem/s 12.756 Melem/s]

blake3_hash_chunk/native
                        time:   [769.04 ns 769.74 ns 770.31 ns]
                        thrpt:  [1.2380 GiB/s 1.2390 GiB/s 1.2401 GiB/s]
blake3_hash_chunk/interpreter
                        time:   [229.14 µs 230.88 µs 232.84 µs]
                        thrpt:  [4.1942 MiB/s 4.2297 MiB/s 4.2618 MiB/s]

ed25519_verify/native/valid
                        time:   [16.618 µs 16.641 µs 16.692 µs]
                        thrpt:  [59.908 Kelem/s 60.094 Kelem/s 60.176 Kelem/s]
ed25519_verify/native/invalid
                        time:   [16.525 µs 16.554 µs 16.599 µs]
                        thrpt:  [60.245 Kelem/s 60.407 Kelem/s 60.516 Kelem/s]
ed25519_verify/interpreter/valid
                        time:   [6.3310 ms 6.3605 ms 6.4322 ms]
                        thrpt:  [155.47  elem/s 157.22  elem/s 157.95  elem/s]
ed25519_verify/interpreter/invalid
                        time:   [6.0230 ms 6.0487 ms 6.0823 ms]
                        thrpt:  [164.41  elem/s 165.32  elem/s 166.03  elem/s]
```

This is slightly apples to oranges comparing AVX512 assembly BLAKE3 implementation on Zen 4 against a basic interpreter
for RV64EM target, but that is how I measure a success: the ceiling is what the hardware is capable of, and we need to
get close to that with whatever VM ends up being used. I do plan to add vector and crypto extensions in the future,
though.

So the initial performance gap is around three orders of magnitude.

## A bit about general approach

When writing code, I'm trying to make it look "obviously correct" whenever possible, especially when complex concepts
are involved. Of course, the domain which I'm working in is inherently complex, so it may not be _that_ obvious in
absolute terms, but I hope you at least get the idea of what I'm trying to say here.

Going back to the topic of RISC-V interpreter, my initial approach was to have an instruction decoder and interpreter
separately. Decoder outputs an enum, and then the interpreter matches on decoded instructions and executes them. I even
extracted M extension from RV64 base in [PR 511] and used composition with a base instruction set to allow more
flexibility and being able to test various permutations quickly, which allowed me to add B extension (including optional
Zbc) in [PR 512] shortly after that. Zbc extension was further optimized with platform-specific intrinsics on
x86-64/aarch64/RV64 in [PR 518].

[PR 511]: https://github.com/nazar-pc/abundance/pull/511

[PR 512]: https://github.com/nazar-pc/abundance/pull/512

[PR 518]: https://github.com/nazar-pc/abundance/pull/518

That simplicity is great for reviewing and understanding code. For example, here is what decoding and execution of
Zbs instructions looks like in the current version of the codebase:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rv64ZbsInstruction<Reg> {
    // Single-Bit Set
    Bset { rd: Reg, rs1: Reg, rs2: Reg },
    Bseti { rd: Reg, rs1: Reg, shamt: u8 },

    // Single-Bit Clear
    Bclr { rd: Reg, rs1: Reg, rs2: Reg },
    Bclri { rd: Reg, rs1: Reg, shamt: u8 },

    // Single-Bit Invert
    Binv { rd: Reg, rs1: Reg, rs2: Reg },
    Binvi { rd: Reg, rs1: Reg, shamt: u8 },

    // Single-Bit Extract
    Bext { rd: Reg, rs1: Reg, rs2: Reg },
    Bexti { rd: Reg, rs1: Reg, shamt: u8 },
}

impl<Reg> const Instruction for Rv64ZbsInstruction<Reg>
where
    Reg: [ const ] Register<Type=u64>,
{
    type Reg = Reg;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        let opcode = (instruction & 0b111_1111) as u8;
        let rd_bits = ((instruction >> 7) & 0x1f) as u8;
        let funct3 = ((instruction >> 12) & 0b111) as u8;
        let rs1_bits = ((instruction >> 15) & 0x1f) as u8;
        let rs2_bits = ((instruction >> 20) & 0x1f) as u8;
        let shamt = ((instruction >> 20) & 0x3f) as u8;
        let funct7 = ((instruction >> 25) & 0b111_1111) as u8;
        let funct6 = ((instruction >> 26) & 0b11_1111) as u8;

        match opcode {
            // R-type instructions
            0b0110011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                match (funct3, funct7) {
                    (0b001, 0b0010100) => Some(Self::Bset { rd, rs1, rs2 }),
                    (0b001, 0b0100100) => Some(Self::Bclr { rd, rs1, rs2 }),
                    (0b001, 0b0110100) => Some(Self::Binv { rd, rs1, rs2 }),
                    (0b101, 0b0100100) => Some(Self::Bext { rd, rs1, rs2 }),
                    _ => None,
                }
            }
            // I-type instructions
            0b0010011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                match (funct3, funct6) {
                    (0b001, 0b001010) => Some(Self::Bseti { rd, rs1, shamt }),
                    (0b001, 0b010010) => Some(Self::Bclri { rd, rs1, shamt }),
                    (0b001, 0b011010) => Some(Self::Binvi { rd, rs1, shamt }),
                    (0b101, 0b010010) => Some(Self::Bexti { rd, rs1, shamt }),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    #[inline(always)]
    fn alignment() -> u8 {
        size_of::<u32>() as u8
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}
```

```rust
impl<Reg, Memory, PC, InstructionHandler, CustomError>
ExecutableInstruction<
    Rv64InterpreterState<Reg, Memory, PC, InstructionHandler, CustomError>,
    CustomError,
> for Rv64ZbsInstruction<Reg>
where
    Reg: Register<Type=u64>,
    [(); Reg::N]:,
{
    #[inline(always)]
    fn execute(
        self,
        state: &mut Rv64InterpreterState<Reg, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, Self, CustomError>> {
        match self {
            Self::Bset { rd, rs1, rs2 } => {
                // Only the bottom 6 bits for RV64
                let index = state.regs.read(rs2) & 0x3f;
                let result = state.regs.read(rs1) | (1u64 << index);
                state.regs.write(rd, result);
            }
            Self::Bseti { rd, rs1, shamt } => {
                let index = shamt;
                let result = state.regs.read(rs1) | (1u64 << index);
                state.regs.write(rd, result);
            }
            Self::Bclr { rd, rs1, rs2 } => {
                let index = state.regs.read(rs2) & 0x3f;
                let result = state.regs.read(rs1) & !(1u64 << index);
                state.regs.write(rd, result);
            }
            Self::Bclri { rd, rs1, shamt } => {
                let index = shamt;
                let result = state.regs.read(rs1) & !(1u64 << index);
                state.regs.write(rd, result);
            }
            Self::Binv { rd, rs1, rs2 } => {
                let index = state.regs.read(rs2) & 0x3f;
                let result = state.regs.read(rs1) ^ (1u64 << index);
                state.regs.write(rd, result);
            }
            Self::Binvi { rd, rs1, shamt } => {
                let index = shamt;
                let result = state.regs.read(rs1) ^ (1u64 << index);
                state.regs.write(rd, result);
            }
            Self::Bext { rd, rs1, rs2 } => {
                let index = state.regs.read(rs2) & 0x3f;
                let result = (state.regs.read(rs1) >> index) & 1;
                state.regs.write(rd, result);
            }
            Self::Bexti { rd, rs1, shamt } => {
                let index = shamt;
                let result = (state.regs.read(rs1) >> index) & 1;
                state.regs.write(rd, result);
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}
```

But that was not so great for performance since in addition to branches during instruction decoding, another match (or
several when composition of multiple instructions is used) needs to be done on something that was just decoded.

You may also notice quite a few generics, that is because interpreter and register/instruction abstractions are generic
over both the base instruction set (RV32/RV64) and the number of registers (E extension, so RV32I/RV64I vs.
RV32E/RV64E). While there is no instruction decoding or execution implemented for RV32 base yet, it should technically
work. Interpreter is also generic over memory, instruction decoder, and handling of syscalls, which is why I'm hoping
the work I'm doing will be useful for the broader Rust ecosystem since I have not seen something that looks exactly like
this yet. Also, neither instruction decoder nor interpreter are in any way specific or dependent on anything
Abundance-specific, only generic reusable code.

Of course, I'm heavily using Rust Nightly features, so it only runs on Nightly and will stay that way for some time.

## Eager instruction decoding

The first obvious thing was when I compared the number of instructions in the contract with the number of instructions
executed when doing BLAKE3 hashing and Ed25519 signature verification. Turns out, due to loops in the code, the number
of instructions executed is far larger, so it makes sense to decode the whole contract upfront and then simply fetch
pre-decoded instructions. That was immediately much better, as can be seen in benchmark results:

```
file/decode-instructions
                        time:   [112.00 µs 113.21 µs 115.70 µs]
                        thrpt:  [8.6431 Kelem/s 8.8333 Kelem/s 8.9288 Kelem/s]

blake3_hash_chunk/interpreter/lazy
                        time:   [200.31 µs 211.64 µs 217.33 µs]
                        thrpt:  [4.4935 MiB/s 4.6143 MiB/s 4.8753 MiB/s]
blake3_hash_chunk/interpreter/eager
                        time:   [81.048 µs 86.829 µs 91.780 µs]
                        thrpt:  [10.640 MiB/s 11.247 MiB/s 12.049 MiB/s]

ed25519_verify/interpreter/lazy
                        time:   [6.7690 ms 6.8320 ms 6.8638 ms]
                        thrpt:  [145.69  elem/s 146.37  elem/s 147.73  elem/s]
ed25519_verify/interpreter/eager
                        time:   [2.3935 ms 2.4013 ms 2.4121 ms]
                        thrpt:  [414.58  elem/s 416.44  elem/s 417.80  elem/s]
```

Though it was still very far from native, despite however flawed that comparison is to begin with.

## Spatial locality of popular instructions

While I've done some more improvements like skipping bounds checks during instruction fetching in most cases in [PR 517]
and combined some shared fields in the same data structure in [PR 520], none of it allowed closing the gap
substantially.

[PR 517]: https://github.com/nazar-pc/abundance/pull/517

[PR 520]: https://github.com/nazar-pc/abundance/pull/520

What I explored then was which instructions are even included in the contract's binary. Turned out, very few relatively
to the available set and with a heavy skew towards a small minority:

```
2637 ld         80 andn       3 bseti
2086 sd         59 sh3add     3 slti
1983 add        46 andi       3 addiw
1266 addi       26 rev8       2 sextb
 974 xor        26 bne        2 slt
 736 rori       23 lui        2 sh
 689 srli       18 lb         2 lh
 593 or         17 beq        2 blt
 561 and        15 srai       1 bexti
 548 slli       15 jal        1 bclri
 526 lbu        14 bltu       1 unimp
 393 auipc      11 bgeu
 318 jalr       10 sw
 260 sb         10 lw
 224 roriw       9 sltiu
 169 sub         7 subw
 127 sltu        5 xori
 121 mulhu       4 sh2add
 120 mul         4 srl
 106 sh1add      4 sll
```

And surely enough, after doing some crude refactoring locally and moving enum variants and interpreter implementation of
popular instructions closer together, I was able to observe double-digit percentage performance improvement.

## Procedural macros alone are not enough

The problem is, I'd really like to keep instruction decoding and execution the way it is. It is very readable and not
tied to any project, it is pretty much what you'd write if you read the spec. But it is bad for performance, so what do
we do? The answer, often, is procedural macros.

My intial plan was to parse enum with instructions and then generate a `macro_rules` for each variant, so I can
recombine them something like this:

```
#[instruction]
enum Custom {
    rv64_add!(),
    rv64_sub!(),
}
```

I tried a few variations of this, but ultimately due to the order of macro expansion, those `rv64_add!()` end up being
expanded last and Rust doesn't allow them to be placed in that position. I've spent some time thinking about something
that looks like Rust and concluded that it can't be done with procedural macro alone.

Thankfully, David Tolnay comes to the rescue again with [syn] and [quote] that do not require to be used within
procedural macros. So the new plan was to parse all files from `build.rs`, generate necessary replacements bits of code
in separate files, which procedural macro will then simply replace original definitions with. And since I want to be
able to still do composition of various extensions, `build.rs` can [emit metadata] with dependencies and locations of
generated files that dependant crates can consume and/or re-export further.

[syn]: https://crates.io/crates/syn

[quote]: https://crates.io/crates/quote

[emit metadata]: https://doc.rust-lang.org/cargo/reference/build-scripts.html#the-links-manifest-key

In the simplest case, slapping `#[instruction]` on instruction definition and decoding implementation and
`#[instruction_execution]` on execution implementation is enough to parse and generate necessary metadata:

```rust
/// RISC-V RV64 instruction
#[instruction]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rv64Instruction<Reg> {
    Add { rd: Reg, rs1: Reg, rs2: Reg },
    // ,,,
}

#[instruction]
impl<Reg> const Instruction for Rv64Instruction<Reg>
where
    Reg: [ const ] Register<Type=u64>,
{
    type Reg = Reg;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        // ...
    }
}
```

```rust
#[instruction_execution]
impl<Reg, Memory, PC, InstructionHandler, CustomError>
ExecutableInstruction<
    Rv64InterpreterState<Reg, Memory, PC, InstructionHandler, CustomError>,
    CustomError,
> for Rv64Instruction<Reg>
where
    Reg: Register<Type=u64>,
    [(); Reg::N]:,
    Memory: VirtualMemory,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
    InstructionHandler: Rv64SystemInstructionHandler<Reg, Memory, PC, CustomError>,
{
    #[inline(always)]
    fn execute(
        self,
        state: &mut Rv64InterpreterState<Reg, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, Self, CustomError>> {
        match self {
            Self::Add { rd, rs1, rs2 } => {
                let value = state.regs.read(rs1).wrapping_add(state.regs.read(rs2));
                state.regs.write(rd, value);
            }
            // ...
        }
    }
}
```

And then, of course, additional call needs to be made in `build.rs`:

```rust
use ab_riscv_macros::process_instruction_macros;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    process_instruction_macros()?;

    Ok(())
}
```

And for all this to be reusable by dependencies it, `package.links` needs to be added to `Cargo.toml`:

```toml
[package]
name = "ab-contract-file"
# ...
links = "ab-contract-file"
```

But the most interesting thing is that this makes it possible to split, merge, reorder, or partially skipping
instructions. As I mentioned earlier, some instructions are more popular than others:

```rust
/// Instructions that are the most popular among contracts
#[instruction(
    reorder = [
        Ld, Sd, Add, Addi, Xor, Rori, Srli, Or, And, Slli, Lbu, Auipc, Jalr, Sb, Roriw, Sub, Sltu,
        Mulhu, Mul, Sh1add,
    ],
    ignore = [
        Rv64Instruction,
        Rv64MInstruction,
        Rv64BInstruction,
        Rv64ZbcInstruction,
    ],
    inherit = [
        Rv64Instruction,
        Rv64MInstruction,
        Rv64BInstruction,
        Rv64ZbcInstruction,
    ],
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopularInstruction<Reg> {}

/// Instructions that are less popular among contracts
#[instruction(
    ignore = [PopularInstruction, Fence, Ecall],
    inherit = [
        Rv64Instruction,
        Rv64MInstruction,
        Rv64BInstruction,
        Rv64ZbcInstruction,
    ],
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotPopularInstruction<Reg> {}
```

With this, we get two enums, where all instructions are reordered and flattened in both definition and implementation.
While at it, I also ignored some instructions that contracts are not supposed to contain like `Fence` and `Ecall`. And
surely enough, performance improved again significantly; even instruction decoding became much faster:

```
file/parse-only         time:   [83.585 µs 85.928 µs 88.935 µs]
                        thrpt:  [11.244 Kelem/s 11.638 Kelem/s 11.964 Kelem/s]
                 change:
                        time:   [−28.603% −27.030% −25.232%] (p = 0.00 < 0.05)
                        thrpt:  [+33.747% +37.042% +40.061%]
file/parse-with-methods time:   [83.389 µs 84.704 µs 86.508 µs]
                        thrpt:  [11.560 Kelem/s 11.806 Kelem/s 11.992 Kelem/s]
                 change:
                        time:   [−29.627% −28.854% −27.861%] (p = 0.00 < 0.05)
                        thrpt:  [+38.621% +40.555% +42.100%]
file/decode-instructions
                        time:   [91.852 µs 92.057 µs 92.373 µs]
                        thrpt:  [10.826 Kelem/s 10.863 Kelem/s 10.887 Kelem/s]
                 change:
                        time:   [−11.233% −9.3143% −7.4060%] (p = 0.00 < 0.05)
                        thrpt:  [+7.9983% +10.271% +12.655%]

blake3_hash_chunk/native
                        time:   [769.88 ns 770.34 ns 770.86 ns]
                        thrpt:  [1.2372 GiB/s 1.2380 GiB/s 1.2387 GiB/s]
blake3_hash_chunk/interpreter/lazy
                        time:   [123.31 µs 125.63 µs 131.19 µs]
                        thrpt:  [7.4437 MiB/s 7.7734 MiB/s 7.9194 MiB/s]
                 change:
                        time:   [−40.658% −38.304% −35.395%] (p = 0.00 < 0.05)
                        thrpt:  [+54.786% +62.086% +68.513%]
blake3_hash_chunk/interpreter/eager
                        time:   [58.122 µs 60.115 µs 63.763 µs]
                        thrpt:  [15.316 MiB/s 16.245 MiB/s 16.802 MiB/s]
                 change:
                        time:   [−10.095% −5.8940% −2.0291%] (p = 0.01 < 0.05)
                        thrpt:  [+2.0711% +6.2632% +11.229%]

ed25519_verify/native   time:   [16.581 µs 16.625 µs 16.743 µs]
                        thrpt:  [59.727 Kelem/s 60.150 Kelem/s 60.310 Kelem/s]
ed25519_verify/interpreter/lazy
                        time:   [4.4544 ms 4.5550 ms 4.7086 ms]
                        thrpt:  [212.38  elem/s 219.54  elem/s 224.50  elem/s]
                 change:
                        time:   [−35.284% −33.827% −32.133%] (p = 0.00 < 0.05)
                        thrpt:  [+47.347% +51.119% +54.522%]
ed25519_verify/interpreter/eager
                        time:   [2.1032 ms 2.1154 ms 2.1251 ms]
                        thrpt:  [470.56  elem/s 472.71  elem/s 475.48  elem/s]
                 change:
                        time:   [−10.465% −8.8065% −7.3122%] (p = 0.00 < 0.05)
                        thrpt:  [+7.8891% +9.6570% +11.688%]
```

Still about two orders of magnitude slower, but it is not three, progress!

Of course, above enums are defined downstream of generic crates since they are specific to contracts, but now anyone can
do similar things without massive copy-pasting and with quick iteration speed.

All this was implemented in [PR 531]. As a bonus, it also [has code pre- and post-processors] because it turns out `syn`
only supports stable Rust syntax and I used very much unstable and even incomplete Nightly features, so I had to replace
things like `impl const Instruction` with `impl cnst::Instruction` and `Reg: [const] Register` with
`Reg: BRCONST+Register` just to keep it syntaxically valid in stable Rust for `syn` to parse successfully and then
reverting it back before writing to files. You can find this hack [here].

[PR 531]: https://github.com/nazar-pc/abundance/pull/531

[has code pre- and post-processors]: https://github.com/nazar-pc/abundance/pull/531/files#diff-8f60208ee893d4d5718247f38061b94263f759d50448245ca9e0a1379b9d1c3e

[here]: https://github.com/nazar-pc/abundance/pull/531/files#diff-8f60208ee893d4d5718247f38061b94263f759d50448245ca9e0a1379b9d1c3e

Overall, I'm quite happy with the way it turned out, the biggest drawback so far is developer experience since going to
definition of these enums in IntelliJ now jumps to generated files, and due to the order of macro expansion and other
compile-time limitations, I don't think there is much that can be done with it for now.

## Next steps for improving performance

The workflow still follows the pattern of separate instruction decoding and execution. Now that I have the machinery
for parsing and generating code while resolving dependencies, it should be possible to combine instruction decoding and
execution in lazy case to skip the extra `match` before execution. It should also be possible to extract execution of
each enum variant into a separate generated function and then store pointers to those functions alongside enum payload,
which opens the possibility for jump-dispatch or indirect threading.

It is still early for me, but I'm optimistic it'll be possible to close the gap of another magnitude order with that,
some instruction fusion and adding support for vector extension. While it wouldn't be possible to take advantage of
vector extension very easily in Nightly Rust directly until [scalable vectors for RISC-V] are supported (thankfully,
there was a push last year for [SVE/SME on aarch64], which RISC-V will benefit from too), LLVM auto-vectorization is
still helpful. I tried compiling contracts with `V` extension and can confirm the code is significantly more compact,
meaning LLVM is doing something implicitly.

[scalable vectors for RISC-V]: https://github.com/rust-lang/rust/issues/133146

[SVE/SME on aarch64]: https://github.com/rust-lang/rust-project-goals/issues/270

If you have other ideas or would like to work on it, please reach out.

## Infrastructure improvements

In [PR 528] and [PR 529] I introduced CI checks for RISC-V targets (primarily Clippy and Miri).

[PR 528]: https://github.com/nazar-pc/abundance/pull/528

[PR 529]: https://github.com/nazar-pc/abundance/pull/529

In [PR 534] I did a substantial amount of work on RISC-V compliance testing by parsing test cases from files in included
in [riscv-non-isa/riscv-arch-test]. While I didn't discover any implementation issues so far, I sleep better knowing
that test coverage has improved. It is only for interpreter, though. I'm still looking for reasonably usable test
vectors for instruction decoding that I can take advantage of. If you know about any, please share with me. In the
process of working on this, it turned out the whole B extension had broken data for many years in that repo, so only
RV64I (RV64E is missing in the repo) and M extension for RV64 is currently being tested.

[PR 534]: https://github.com/nazar-pc/abundance/pull/534

[riscv-non-isa/riscv-arch-test]: https://github.com/riscv-non-isa/riscv-arch-test

## Improvements to contracts ABI

Well, that was a long update already, but it turns out working with contracts was a bit more unpleasant than expected,
so I've done some work there as well to improve the status quo.

First, with [PR 498] it is now possible to not only write, but also read `#[output]` values. There were a few bugs found
as well, which I fixed with additional test cases added accordingly. This made calling RISC-V contracts for benchmarking
purposes directly much more pleasant. In fact, I'm now on track to removing the need to specify `#[input]` and
`#[output]` on contract method arguments, which I think will be a positive change, especially for the simplest cases.
The other arguments like slots will still need annotations since there is no way to infer them.

[PR 498]: https://github.com/nazar-pc/abundance/pull/498

And I changed ABI in [PR 508], which I think makes things simpler to explain and simpler in the implementation and
removes the need to use null pointers in some cases, which bothered me for a while. I do not see the need to change ABI
any further from now on, but we'll see if something else gets discovered to change my mind.

[PR 508]: https://github.com/nazar-pc/abundance/pull/508

## Upcoming plans

With that unexpectedly lengthy update out of the way, I think I'll pause with RISC-V changes for a bit. It feels more
and more like a grind rather than something exciting after working with it for a few weeks. While I didn't wrap the
interpreter into the execution environment yet, it shouldn't take too long once I actually need it with the refactoring
I have done recently.

I'll probably focus on sharding for a bit again, after all, the global history and shard confirmation still lacks
implementation and has some open design questions I need to address before intermediate and leaf shards can be
introduced and for the whole thing to look like something more than another typical basic blockchain.

I'll be happy to chat about anything you've read here on [Zulip] and will be back with another update once I have more
to share about the progress.

[Zulip]: https://abundance.zulipchat.com/
