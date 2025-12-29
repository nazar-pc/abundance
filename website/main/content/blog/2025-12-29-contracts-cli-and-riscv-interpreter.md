---
title: Contracts CLI and RISC-V interpreter
date: 2025-12-29
draft: false
description: Exploring ELF and RISC-V with prototypes of key components
tags: [ status-update ]
authors: [ nazar-pc ]
---

Switching to something that I thought would be more fun, I decided to look into ELF and RISC-V last week or so. I
learned more than I wanted and managed to achieve a few key deliverables:

* design initial CLI for building/converting/verifying contract files
* define and implement a contract file format
* implement a simple RISC-V interpreter

Let's look into each of those in more detail.

<!--more-->

## Initial CLI for building/converting/verifying contract files

I spent quite some time in the past thinking about how the contracts should be built and what the developer experience
about that would look like. Ideally, the developer would just call `cargo build` and get a file they can upload to the
blockchain. Unfortunately, things are not quite that simple, at least for now, and there are multiple reasons for that.

The first reason is that there isn't an official target that would fit the use case for contracts perfectly, which means
a custom target specification is needed, which involves custom CLI options and requirement of a nightly Rust toolchain.
Not only that, the standard library is not available for custom targets, and there is no way to specify that it needs to
be built on demand, which requires more CLI options. And on top of everything I really wanted the developer experience
writing and interacting with contracts look as "normal" as possible, meaning not requiring special crate type
definitions in the `Cargo.toml` and having extra build artifacts when a contract is used as a dependency, which leads to
even more CLI options and the need to build with `cargo rustc` instead of `cargo build`.

For my experiments I used a command that looked something like this:

```bash
cargo rustc \
    --crate-type cdylib \
    -Z build-std=core \
    --package ab-example-contract-flipper \
    --features ab-example-contract-flipper/guest \
    --profile production \
    --target crates/contracts/riscv64em-unknown-none-abundance.json
```

I'm sure it is easy to see that it is not fun to deal with all the time. Not to mention that anyone using it would have
to have an up-to-date version if the target specification file somewhere nearby at all times.

I decided to implement a cargo extension CLI `cargo ab-contract` that would simplify the process of building contracts
and provide a more convenient experience for developers. The first step taken in [PR 483] was to implement `convert`
command that takes ELF `cdylib` as an input and produces a contract file as an output, which was extended with `verify`
command in [PR 493] and `build` command that replaces that messy `cargo rustc` command followed by
`cargo ab-contract convert` with a simple `cargo ab-contract build` in [PR 494].

[PR 483]: https://github.com/nazar-pc/abundance/pull/483

[PR 493]: https://github.com/nazar-pc/abundance/pull/493

[PR 494]: https://github.com/nazar-pc/abundance/pull/494

Eventually I plan to implement `recover` command that would take a contract file as an input and produce an ELF
`cdylib` that would behave similarly to the original one, which may be useful for debugging purposes with traditional
tooling, but it is not 100% clear if that would ever be actually necessary. We'll see.

## Contract file format

Since I mentioned multiple times in the past that the plan was to use ELF files, you might be wondering why the
conversion step is necessary? Well, I spent a lot of time researching and experimenting with ELF RISC-V `cdylib` files
and what they look like. Turns out there is A LOT to it, and it is quite non-trivial to process them correctly, not to
mention that they support so many potential things that will never be needed. On top of that, there is a size concern.
For simple contracts various sections and headers ELF files typically contain are just dead weight, and with contract
metadata included in statics, some of it ends up being duplicated in the output file.

I also spent some time looking into what PolkaVM does and some other projects and concluded that it would be beneficial
to have something way simpler that would be trivial to parse and would only contain the bare minimum necessary.

While looking at the file format, I was also thinking about interpreter/static binary recompiler implementation and the
way memory is supposed to be organized during execution. PolkaVM has an interesting WASM-like design where address space
for code and data are separate and code can't be read, only executed. I decided to not go that way to make the contract
file as close to the original ELF `cdylib` as possible and to make the conversion process dead simple.

This also raises the question about what features should even be supported from the developer perspective. For example,
thread-local storage (TLS) is not useful in a single-threaded environment, I decided early on to not have syscalls and
dynamic memory allocation, but should global mutable statics be available? I ultimately decided that no global mutable
statics should be supported. Rather, a generous amount of stack space (by blockchain standards) will be given during
contract execution, and that is all code will have to work with (in addition to input/output arguments of the contract
call). I learned to dislike global mutable state of all kinds over the years, so this is in-line with my evolved
personal preferences.

So what we're left with in terms of what the file should contain is read-only data (constants, read-only statics,
contract metadata) and code. Code can remain mutable, but nothing outside the code section will be executable.

The conversion process then consists of a bunch of checks to ensure the ELF file doesn't contain what can't be supported
by the contract, then a small header is written first followed by a read-only section and code section from the ELF
file. I made sure to preserve relative offsets between read-only data and code sections such that no fixups are needed
for execution and reversing is still possible.

The header right now looks like this:

```rust
/// Header of the contract file
#[derive(Debug, Clone, Copy, PartialEq, Eq, TrivialType)]
#[repr(C)]
pub struct ContractFileHeader {
    /// Always [`CONTRACT_FILE_MAGIC`]
    pub magic: [u8; 4],
    /// Size of the read-only section in bytes as stored in the file
    pub read_only_section_file_size: u32,
    /// Size of the read-only section in bytes as will be written to memory during execution.
    ///
    /// If larger than `read_only_section_file_size`, then zeroed padding needs to be added.
    pub read_only_section_memory_size: u32,
    /// Offset of the metadata section in bytes relative to the start of the file
    pub metadata_offset: u32,
    /// Size of the metadata section in bytes
    pub metadata_size: u16,
    /// Number of methods in the contract
    pub num_methods: u16,
    /// Host call function offset in bytes relative to the start of the file.
    ///
    /// `0` means no host call.
    pub host_call_fn_offset: u32,
}
```

It is then followed by a bunch of pointers to exported contract methods:

```rust
/// Metadata about each method of the contract that can be called from the outside
#[derive(Debug, Clone, Copy, PartialEq, Eq, TrivialType)]
#[repr(C)]
pub struct ContractFileMethodMetadata {
    /// Offset of the method code in bytes relative to the start of the file
    pub offset: u32,
    /// Size of the method code in bytes
    pub size: u32,
}
```

This is all that is needed to load read-only data and code into memory, find/decode contract metadata (which tells us
about all available methods) and where to find them in memory for execution. I even wrote a simple CLI to call such
contracts.

`host_call_fn_offset` is a special case, it tells us where the host call function is located in memory. Since I didn't
want to use syscalls, the host call is simply an external import in the ELF file, which is proxied through the exported
function. The exported function has a distinct assembly that can be rewritten during execution into something that acts
as a host call, while loading ELF `cdylib` in a regular RISC-V process allows making host calls through a regular
mechanism shared libraries normally use. Feels a bit hacky, so let me know if you know something better.

The code section is parsed as a series of RISC-V instructions and if unexpected instructions are encountered, like
`ecall` used for making syscalls, the whole contract is rejected as invalid.

I'm still thinking whether I should make it a requirement for contracts to be compressed with Zstandard. If so, it would
be possible to include the padding that sometimes appears between read-only data and code sections in the file size,
such that when the contract is decompressed, it doesn't need any post-processing to be loaded into memory, and the
decompressed data would already have the correct memory layout. From what I found, it should be fine security-wise, but
I am still hesitant for some reason. Thankfully, there is a [rust decompression crate], so I wouldn't have to mess with
bindings if I go that route.

[rust decompression crate]: https://github.com/KillingSpark/zstd-rs

Initial file format definition landed together with `cargo ab-contract` in [PR 483] with some fixes in [PR 485], parsing
of the new file was implemented in [PR 487] and that was used for verification post-conversion from ELF `cdylib`
in [PR 493] along with instruction parsing and verification.

[PR 485]: https://github.com/nazar-pc/abundance/pull/485

[PR 487]: https://github.com/nazar-pc/abundance/pull/487

## RISC-V interpreter

To test all of the above, I needed a way to run RISC-V code. An interpreter is a great way to start, and I was
pleasantly surprised by both how simple RISC-V really is compared to something like x86-64 and how capable LLMs are
generating initial mostly working prototype and lengthy tests for it.

I started with a new target specification in [PR 481] since I needed to be able to produce the files first. I was
actually running ELF files directly at first, which helped me to explore and design my own file format. Initial
definition of registers and instructions landed in [PR 483] with the file format introduction. To make it potentially
more usable outside the project, I refactored registers in [PR 486] so not just RV64E, but also RV64I base ISA is
supported. After all, the only difference between the two is the number of general purpose registers and literally
nothing else.

[PR 481]: https://github.com/nazar-pc/abundance/pull/481

[PR 486]: https://github.com/nazar-pc/abundance/pull/486

Calling some methods incorrectly, I quickly discovered magic instruction `0xc0001073` that `objdump` read as `unimp`,
which various compilers canonically use as invalid instruction for panics, so I added support for that in [PR 488] too.
Encouraged by how helpful LLMs were in generating instruction parser, I used them to generate almost two thousand lines
of tests in [PR 489]. While I didn't audit the 100% of tests, they look good enough for now.

[PR 488]: https://github.com/nazar-pc/abundance/pull/488

[PR 489]: https://github.com/nazar-pc/abundance/pull/489

With all those preparations I was finally able to land an interpreter I was toying with for a few days in a local branch
in [PR 490]. It is a very basic textbook interpreter that decodes and executes one instruction at a time. Low
performance, no gas metering, etc., but it works and can be built upon or used as a reference.

[PR 490]: https://github.com/nazar-pc/abundance/pull/490

## Shard allocation follow-up

[The previous update] was about the initial shard allocation implementation, which I briefly continued before switching
to RISC-V-related business.

[The previous update]: ../2025-12-19-permissionless-assignments-of-farmers-to-shards

As mentioned there, I wasn't quite happy with a shard rotation interval measured in slots, so I switched it to beacon
chain blocks in [PR 480], which was a big simplification, especially to intermediate and leaf shards implementation when
time comes. I missed tying allocation to the public key hash initially and fixed it in [PR 482].

[PR 480]: https://github.com/nazar-pc/abundance/pull/480

[PR 482]: https://github.com/nazar-pc/abundance/pull/482

The last improvement is a very basic algorithm for reducing the total set of unique history sizes used per plot
in [PR 479]. It is functional but has a high chance of resulting in too much replotting and also might cause recent
history being significantly underrepresented compared to Subspace. I decided to not overengineer it for now, but it'd
be nice to calculate the probabilities and decide what the algorithm should look like long-term and how the reference
implementation should behave when it comes to replotting.

[PR 479]: https://github.com/nazar-pc/abundance/pull/479

## Upcoming plans

With a basic interpreter in place, it is still not enough to run contracts. An execution environment needs to be
implemented around it similarly to the already existing native execution environment. It'll probably take a couple of
weeks before I have that, but there are zero unknowns about its feasibility at this point.

I'd like to write some benchmarks and see what kind of performance I get and how difficult it would be to make it
half-decent with minimal effort.

This will keep me occupied for a while, I'll decide when to tackle gas metering and other things later, though I have
been doing some research on that already.

This week felt more lively than usual, which I'm quite happy about. We'll see how the next one goes.

It has actually been almost exactly one year since the project formally started. I'll be writing a post summarizing
the progress so far and the road ahead, which with some luck will become a yearly occurrence for many years to come.

[Zulip] is where you can find me to chat about anything related to this update or the project in general.

[Zulip]: https://abundance.zulipchat.com/
