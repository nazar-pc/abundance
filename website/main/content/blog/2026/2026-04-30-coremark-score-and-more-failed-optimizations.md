---
title: CoreMark score and more failed optimizations
date: 2026-04-30
draft: false
description: New releases, CoreMark runner and interpreter optimizations that hit LLVM scalability constraints
tags: [ status-update ]
authors: [ nazar-pc ]
---

Another two weeks or so wrestling with compilers. Not always successful, but numerous lessons were learned in the
process, which I'll take as a (small) win.

On the bright side, there were new releases of RISC-V crates with more improvements, and there is now a CoreMark runner,
which allows somewhat objective comparison of interpreter performance against alternatives.

<!--more-->

## New releases

This time I shipped new releases of [ab-riscv-primitives], [ab-riscv-interpreter] and related macro crates.

[ab-riscv-primitives]: https://crates.io/crates/ab-riscv-primitives

[ab-riscv-interpreter]: https://crates.io/crates/ab-riscv-interpreter

Feature-wise, there are mostly API changes that make it possible to use a custom register file and even register type.
For example, contracts are compiled in a way that avoids `gp` and `tp` registers, so it is possible to reject
instructions that try to use them at decoding time and reduce the register file size accordingly.

I fixed multiple decoding issues for the Zcmp instructions. While RISC-V Architectural Certification Tests do not exist
for it and will not be for a while, CI now runs tests with actual code compiled that leverages those instructions.
Together with greatly extended code coverage, I think it is correct, so I added it to a set of extensions used by
contracts by default.

A few places also needed fixes to properly support compressed instructions, and there were some minor performance
optimizations here and there.

See individual changelogs for details if you're curious.

With support for atomic instructions (not sure how soon), I'll be increasing versions from `0.0.x` to `0.x.y`.

## CoreMark runner

I was re-reading [WASMI 1.0 blog post] recently and noted [CoreMark] scores there. CoreMark is a well-known benchmark,
so in [PR 665] I introduced a runner that runs it under the interpreter to get a number as well.

[WASMI 1.0 blog post]: https://wasmi-labs.github.io/blog/posts/wasmi-v1.0/

[CoreMark]: https://github.com/eembc/coremark

[PR 665]: https://github.com/nazar-pc/abundance/pull/665

Here are the numbers on AMD Threadripper 7970X CPU (the benchmark is single-threaded):

```
2K performance run parameters for coremark.
CoreMark Size    : 666
Total ticks      : 13990585
Total time (secs): 13
Iterations/Sec   : 1538
Iterations       : 20000
Compiler version : GCC13.2.0
Compiler flags   : -O3 -march=rv64imc_zba_zbb_zbs -mabi=lp64
Memory location  : STACK
seedcrc          : 0xe9f5
[0]crclist       : 0xe714
[0]crcmatrix     : 0x1fd7
[0]crcstate      : 0x8e3a
[0]crcfinal      : 0x382f
Correct operation validated. See readme.txt for run and reporting rules.
Host elapsed: 21.848 s
```

I'll be tracking it going forward alongside contract benchmarks when measuring the performance impact of future changes.

## SROA and LLVM limitations

I described [a few failed attempts at performance optimizations] last time, however well intended they were. This time I
tried yet another one that resulted in ~2x performance regression instead. It is a sound design overall, and it should
have worked, but I believe I hit [LLVM limitations] this time that should be possible to alleviate, at least eventually.

[a few failed attempts at performance optimizations]: ../2026-04-16-how-to-not-make-interpreter-faster

[LLVM limitations]: https://discourse.llvm.org/t/register-spilling-with-a-large-switch/90664?u=nazar-pc

The idea is relatively basic: since we uncovered store-to-load conflicts (which is a hardware-level issue) as the
culprit of performance issues, we can rewrite the code in such a way that the hottest RISC-V registers will end up
becoming native registers in the compiled machine code.

Here is what a typical execution loop looks like:

```rust
pub fn execute<Regs, Memory, IF>(
    state: &mut BasicInterpreterState<Regs, (), Memory, IF, IgnoreEcallSystemInstructionHandler>,
) -> Result<(), ExecutionError<u64>>
where
    Regs: RegisterFile<<ContractInstruction as Instruction>::Reg>,
    Memory: VirtualMemory,
    IF: InstructionFetcher<ContractInstruction, Memory>,
{
    loop {
        let instruction = match state.instruction_fetcher.fetch_instruction(&state.memory)? {
            FetchInstructionResult::Instruction(instruction) => instruction,
            FetchInstructionResult::ControlFlow(ControlFlow::Continue(())) => {
                continue;
            }
            FetchInstructionResult::ControlFlow(ControlFlow::Break(())) => {
                break;
            }
        };

        match instruction.execute(
            &mut state.regs,
            &mut state.ext_state,
            &mut state.memory,
            &mut state.instruction_fetcher,
            &mut state.system_instruction_handler,
        )? {
            ControlFlow::Continue(()) => {
                continue;
            }
            ControlFlow::Break(()) => {
                break;
            }
        }
    }

    Ok(())
}
```

With `fetch_instruction` being just an unchecked read and `Instruction::execute()` fully inlined with simple
instructions inside, compiler should be able to optimize this really well. One issue, however, is that all inputs come
through `state` variable as memory references, including the register file (`state.regs`). This results in all register
accesses being memory accesses (slow) rather than register accesses (almost free).

The above state is after recent refactoring in [PR 655], which allows the following semantically equivalent
transformation:

[PR 655]: https://github.com/nazar-pc/abundance/pull/655

```rust
pub fn execute<Regs, Memory, IF>(
    state: &mut BasicInterpreterState<Regs, (), Memory, IF, IgnoreEcallSystemInstructionHandler>,
) -> Result<(), ExecutionError<u64>>
// Same as before
{
    // Copy registers to the stack
    let mut regs = state.regs;

    loop {
        // Same as before

        match instruction.execute(
            &mut regs,
            &mut state.ext_state,
            &mut state.memory,
            &mut state.instruction_fetcher,
            &mut state.system_instruction_handler,
        )? {
            // Same as before
        }
    }

    // Copy back into the state
    state.regs = regs;

    Ok(())
}
```

What we're doing here is allowing LLVM to reason about the contents of the register file in more detail and potentially
apply SROA (Scalar Replacement of Aggregates) optimization. SROA is a fancy way of saying that `regs` as a struct can be
replaced with a set of variables, each corresponding to a `regs`'s field. If some of those fields end up being "hot",
they may end up being stored in the native CPU registers.

The key for this optimization is for LLVM to reason about aliasing rules. And with an explicit argument of a fully
recursively inlined function body (at least as far as register file access is concerned), this should, in principle, be
doable.

Then register file implementation methods instead of accessing a flat array of registers look something like this:

```rust
/// Registers used by contracts
#[derive(Debug, Default, Clone, Copy)]
pub struct ContractRegisters {
    ra: u64,
    sp: u64,
    s0: u64,
    a0: u64,
    a1: u64,
    a2: u64,
    a3: u64,
    cold_registers: [u64; 22],
}

impl ContractRegisters {
    /// Get a reference to the SROA-friendly version of registers to be stored on the stack
    #[inline(always)]
    pub fn stack_registers(&mut self) -> StackRegisters<'_> {
        StackRegisters {
            ra: self.ra,
            sp: self.sp,
            s0: self.s0,
            a0: self.a0,
            a1: self.a1,
            a2: self.a2,
            a3: self.a3,
            original: self,
        }
    }
}

/// This implementation is designed to be friendly to SROA (Scalar Replacement of Aggregates) pass
/// in LLVM that promotes hot registers to native CPU registers for better performance
#[derive(Debug)]
pub struct StackRegisters<'a> {
    ra: u64,
    sp: u64,
    s0: u64,
    a0: u64,
    a1: u64,
    a2: u64,
    a3: u64,
    original: &'a mut ContractRegisters,
}

impl Drop for StackRegisters<'_> {
    #[inline(always)]
    fn drop(&mut self) {
        self.original.ra = self.ra;
        self.original.sp = self.sp;
        self.original.s0 = self.s0;
        self.original.a0 = self.a0;
        self.original.a1 = self.a1;
        self.original.a2 = self.a2;
        self.original.a3 = self.a3;
    }
}

impl const RegisterFile<ContractRegister> for StackRegisters<'_> {
    #[inline(always)]
    fn read(&self, reg: ContractRegister) -> u64 {
        match reg {
            ContractRegister::Zero => {
                // Always zero
                0
            }
            ContractRegister::Ra => self.ra,
            ContractRegister::Sp => self.sp,
            ContractRegister::S0 => self.s0,
            ContractRegister::A0 => self.a0,
            ContractRegister::A1 => self.a1,
            ContractRegister::A2 => self.a2,
            ContractRegister::A3 => self.a3,
            reg => {
                cold_path();
                // SAFETY: register offset is always within bounds
                *unsafe {
                    self.original
                        .cold_registers
                        .get_unchecked(usize::from(reg as u8))
                }
            }
        }
    }

    #[inline(always)]
    fn write(&mut self, reg: ContractRegister, value: u64) {
        match reg {
            ContractRegister::Zero => {
                // Writes are ignored
            }
            ContractRegister::Ra => {
                self.ra = value;
            }
            ContractRegister::Sp => {
                self.sp = value;
            }
            ContractRegister::S0 => {
                self.s0 = value;
            }
            ContractRegister::A0 => {
                self.a0 = value;
            }
            ContractRegister::A1 => {
                self.a1 = value;
            }
            ContractRegister::A2 => {
                self.a2 = value;
            }
            ContractRegister::A3 => {
                self.a3 = value;
            }
            reg => {
                cold_path();
                // SAFETY: register offset is always within bounds
                *unsafe {
                    self.original
                        .cold_registers
                        .get_unchecked_mut(usize::from(reg as u8))
                } = value;
            }
        }
    }
}
```

Here registers `ra`, `sp`, `s0`, `a0`, `a1`, `a2` and `a3` are "hot" and the rest are cold. This specific field access
pattern seems essential for LLVM to promote fields to the equivalent of local variables, while implementations of
instructions do not need to think about it. I created `StackRegisters` to not copy `cold_registers` onto the stack and
to automatically save updated registers back into `state` on drop of the temporary variable.

The net result was ~2-3x performance reduction. Again.

Turns out, `Instruction::execute()` has way too many branches for LLVM to reason about, so it does SROA pass and `regs`
disappears as a unit, but I do not get the native register promotion. In fact, what I got instead was a huge amount of
register spillage, which made the memory issue more severe instead of solving it.

Now I would like to get the attention from LLVM maintainers/developers to do something about it because this looks like
the most efficient way to implement things and the most logical too, but LLVM is unable to handle it (yet).

## Conclusion (updated)

With that, I think I'm putting a pin on interpreter optimizations for now. It is possible to work around those
limitations with platform-specific inline assembly, but handling it in a platform-independent way while retaining
composability and sane API is not really feasible from my point of view.

I will probably keep implementing more RISC-V extensions like atomics, but performance-wise I'll settle on this for now.

## Upcoming plans

I'll be switching back to mentally intensive thinking about sharding design with the relationship between shards on
different levels, so I can finally introduce intermediate and leaf shards in addition to the beacon chain. It might take
a while, but I'll be posting updates once there is something notable to share.

In the meantime, with any ideas or suggestions you can find me on [Zulip] as usual.

[Zulip]: https://abundance.zulipchat.com/
