---
title: Major RISC-V improvements
date: 2026-04-04
draft: false
description: RV32 support, passing of certification tests and experimental vector extension support
tags: [ status-update ]
authors: [ nazar-pc ]
---

I spent a lot of time working on the RISC-V interpreter in the last few weeks and got some substantial improvements.
Performance is largely the same, but there are still many notable changes both for the project and potential external
users now that the crates were [published on crates.io] (spoiler, new versions were published since).

[published on crates.io]: ../2026-03-14-first-crates-on-crates.io

<!--more-->

## Extensible interpreter state

Previously only general purpose registers (GPRs) were supported since none of the extensions needed anything else. But
there are in fact other kinds of registers and other architectural states that might need to be stored in the
interpreter. For example, Control and Status Registers (CSRs), vector registers, floating-point registers are all
distinct.

Since the interpreter is generic and composable, I wanted a flexible way to introduce all of those on a case-by-case
basis without forcing users that only need GPRs to waste memory storing registers they'll not use.

Eventually, I came up with the solution: add a single field to the interpreter state that is generic and let interpreter
users decide what it contains:

```rust
#[derive(Debug)]
pub struct InterpreterState<
    Reg,
    ExtState,
    Memory,
    IF,
    InstructionHandler,
    CustomError = CustomErrorPlaceholder,
>
where
    Reg: Register,
    [(); Reg::N]:,
{
    /// General purpose registers
    pub regs: Registers<Reg>,
    /// Extended state.
    ///
    /// Extensions might use this to place additional constraints on `ExtState` to require
    /// additional registers or other resources. If no such extension is used, `()` can be used as
    /// a placeholder.
    pub ext_state: ExtState,
    /// Memory
    pub memory: Memory,
    /// Instruction fetcher
    pub instruction_fetcher: IF,
    /// System instruction handler
    pub system_instruction_handler: InstructionHandler,
    /// Custom error phantom data
    pub custom_error: PhantomData<CustomError>,
}
```

For example, [PR 583] introduced interpreter support for Zicsr extension (decoupled from RV64 in [PR 590]) and its
implementation looks like this:

[PR 583]: https://github.com/nazar-pc/abundance/pull/583

[PR 590]: https://github.com/nazar-pc/abundance/pull/590

```rust
#[instruction_execution]
impl<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>
ExecutableInstruction<
    InterpreterState<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>,
    CustomError,
> for ZicsrInstruction<Reg>
where
    Reg: Register,
    [(); Reg::N]:,
    ExtState: Csrs<Reg, CustomError>,
{
    #[inline(always)]
    fn execute(
        self,
        state: &mut InterpreterState<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, CustomError>> {
        // ..
    }
}
```

Note `ExtState: Csrs<Reg, CustomError>,` bound, saying that whatever `ExtState` is, it must implement `Csrs` trait.

This way, depending on circumstances, CSRs can be stored in some kind of map or just a few struct fields if the number
of CSRs supported in a particular setup is tiny. Whatever makes the most sense in each situation, the API is flexible
enough to support it. And for those who don't need extra state, `ExtState` can be `()`.

## Vector extension support

Now that you know that Zicsr extension is supported, which is a dependency, I started working on vector extension,
specifically Zve64x. Well, I started with Zve64x, but the implementation is generic over both element width `ELEN` and
the vector register width `VLEN`, so it technically supports both Zve64x and Zve32x at any supported vector length.

It was a long process, despite leveraging Claude to spit out a bunch of code in the process. I had to slice the
implementation into parts just like with instruction decoding and ended up with a chain of 11 PRs that ended
with [PR 614]. It was over 35k lines of code, and that was the only way to make sense of it and for AI bots to review
them properly. I use both GitHub Copilot and Graphite, and Graphite refuses to review PRs that are too large. This is
excluding multiple preparation and refactoring PRs.

[PR 614]: https://github.com/nazar-pc/abundance/pull/614

As you can imagine, there were a lot of generics involved, including const generics, and at some point I reached the
limits of what Rustc supports today. It is mostly not bad and constraints users to only valid `ELEN` and `VLEN`
combinations, but it could have been nicer if Rustc was more powerful.

So the good news is that Zve64x is technically implemented and even seems to pass smoke tests with certain vector
lengths. Unfortunately, though, there are still bugs, and it fails to function properly with some vector lengths, at
least from the tests I have done.

## Testing improvements

So Zve64x tests are failing when running real code, now what? Well, with 35k lines of code and hundreds of instructions,
with most having multiple variations, it is not really feasible to debug this by hand.

But I mentioned I was running previous instructions against [RISC-V Architectural Certification Tests] (ACTs), or at
least a hacked version of the previous iteration. Maintainers since came up with ACT4, and I decided to support it
properly now, so I get easy access to the full suite of upstream tests to check my implementation against. Especially
since even with the older version I [couldn't run] some tests, and it wasn't sustainable anyway.

[RISC-V Architectural Certification Tests]: https://github.com/riscv/riscv-arch-test

[couldn't run]: https://github.com/riscv/riscv-arch-test/issues/860

So I decided to implement proper support for ACT4, meaning to run ELFs it produces. It took some time trying to make
sense of it and some LLM help, but I got it working in [PR 618] and replaced the previous hacky approach. As a result,
all extensions except Zve64x (which doesn't have upstream tests yet) are now tested and passing 🥳

[PR 618]: https://github.com/nazar-pc/abundance/pull/618

Zve64x is [reportedly coming soon], but until then I don't have a productive route to debug the implementation.

[reportedly coming soon]: https://github.com/riscv/riscv-arch-test/issues/593

There are a few difficulties with it, though, which I reported upstream. One of them is the heavy set of dependencies
required to compile ELF files, and I didn't want to store megabytes of binaries in the repository either. So I ended up
writing a Dockerfile for a proper build in a relatively minimal environment, which took an embarrassing amount of time
to debug in CI. I [submitted a PR] to upstream, so hopefully soon I can just pull it from there.

[submitted a PR]: https://github.com/riscv/riscv-arch-test/pull/1161

With these tests I found and fixed a few implementation issues ([PR 601], [PR 602]), while the rest "just worked."

[PR 601]: https://github.com/nazar-pc/abundance/pull/601

[PR 602]: https://github.com/nazar-pc/abundance/pull/602

## RV32 support

Now that I have tests, it was just an evening of LLM interrogation to add RV32 support with all the extensions RV64
already had in [PR 619]. And this time, since I have certification tests for it passing too, I have a decent level of
confidence that it works the way it should. That was surprisingly easy, and I hope will be useful to someone.

[PR 619]: https://github.com/nazar-pc/abundance/pull/619

Going forward, adding support for extensions that ACT4 supports will be much smoother.

## New releases

I published 0.0.2 releases of both [primitives] and [interpreter] crates on crates.io, so if you're interested, take a
look. I also improved docs/metadata a bit and added changelog entries that I plan to maintain going forward.

[primitives]: https://crates.io/crates/ab-riscv-primitives

[interpreter]: https://crates.io/crates/ab-riscv-interpreter

## Upcoming plans

Now that I wait for Zve64x to be supported in ACT4, I think I'll implement some architectural performance improvements
for RISC-V interpreter, and then I'll probably switch back to consensus work figuring out the relationship between
shard blocks.

That was a relatively short description of long days of work. I'll be back with another update once I have more things
to share. Until then, thanks for reading and feel free to reach out on [Zulip]!

[Zulip]: https://abundance.zulipchat.com/
