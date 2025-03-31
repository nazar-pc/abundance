---
title: Building contract files
date: 2025-03-31
draft: false
description: Latest progress on execution environment, building contracts and other things
tags: [ status-update ]
authors: [ nazar-pc ]
---

The majority of the last two weeks I've been busy with the installation of the antivirus system update for my immune
system. It was neither pleasant nor quick, but now that it is slowly approaching 100%, I'm back with another update of
what I managed to do since the last update.

<!--more-->

I was able to conduct a few interviews with people of different background that helped to improve documentation
([PR 134], [PR 138], [PR 141], [PR 144]). Overall good discussions, but no major issues were uncovered so far. Method
context seems to be a more difficult concept to grasp that I don't think can be avoided, but hopefully the latest
revision of the book helps with understanding a bit.

[PR 134]: https://github.com/nazar-pc/abundance/pull/134

[PR 138]: https://github.com/nazar-pc/abundance/pull/138

[PR 141]: https://github.com/nazar-pc/abundance/pull/141

[PR 144]: https://github.com/nazar-pc/abundance/pull/144

## Contracts as ELF files

I've spent a lot more time tinkering with ELF files and various options of the compiler and linker to see what would be
the most ergonomic way to produce compact files without requiring user to learn some custom tooling. As a result, I
came to the conclusion that a shared library is probably the best path going forward.

Contracts inherently behave like libraries that have multiple callable entrypoints, so a standard shared library is a
good fit because it already contains all the necessary details about exported symbols. Another thing I really wanted to
avoid is custom syscalls or other implicit "environment" for contracts to interact with. Using import symbols might be a
good middle ground that avoids hardcoding a specific implementation, allowing VM to use whatever makes sense in its
context.

With that in mind, [PR 137] introduced addition CI jobs that build contracts against a custom `no_std` target. It is an
x86-64 target for now, so I can play with its output locally, but eventually it will change to 64-bit RISC-V. While it
is bare-metal-like and single-threaded, it does output a shared library in ELF format, which can be loaded with
`dlopen()` and whose functions can be called directly. [PR 136] and [PR 142] introduced further tweaks to it.

[PR 137]: https://github.com/nazar-pc/abundance/pull/137

[PR 136]: https://github.com/nazar-pc/abundance/pull/136

[PR 142]: https://github.com/nazar-pc/abundance/pull/142

The building process for `ab-example-contract-ft` in the workspace looks something like this (verbose, but completely
standard):

```bash
cargo rustc --crate-type cdylib -Z build-std=core \
    --package ab-example-contract-ft \
    --features ab-example-contract-ft/guest \
    --profile contract \
    --target x86_64-unknown-none-abundance.json
```

Profile `contract` is custom and essentially enables LTO and strips symbols to produce a compact "production" output in
the form of `ab_example_contract_ft.contract` file. `-Z build-std=core` [will hopefully not be required at some point],
similarly maybe `--crate-type cdylib` might be the default for the custom target somehow, so it doesn't need to be
specified explicitly.

[will hopefully not be required at some point]: https://github.com/rust-lang/wg-cargo-std-aware/issues/95

The file still contains an annoying `.comment` section with Rust, LLVM and LLD versions, which can be stripped with:

```bash
strip -R .comment ab_example_contract_ft.contract
```

I looked for ways to avoid it without this extra step, but didn't find any.

So how big are the output files you might be wondering? Reasonably small actually, but ELF has relatively large 64-byte
sections with a bunch of zeroes that increase the size a bit. For `flipper` example the size is ~1.8 kiB before manual
stripping of `.comment` section and ~2,7 kiB for `ft` contract.

The good thing is that it compresses well with zstd, down to ~550 bytes for `flipper` and ~1100 bytes for `ft`
contract (after `.comment` stripping), which is much closer to what I'd like to see. I think zstd compression is what
will be used for contracts in practice since there is some repetition and a bunch of zeroes in the ELF file.

The huge benefit of this design is that it'd be possible to use all the standard tooling with these contracts. Regular
Cargo/Rustc or even other compilers to produce an ELF file, no custom tooling is required (though it might be more
convenient). I can also imagine a custom app that can load such a contract file plus encoded method call (dumped from
block explorer, for example) to do step-by-step debugging in gdb/lldb. All standard disassembling and optimization tools
should also work with these contracts.

## Transactions

I did some refactoring for transactions in preparation for a basic implementation of the transaction pool in [PR 143],
but ultimately didn't have time to do anything significant there. I did spend a lot of time thinking about it and should
have more progress in the coming weeks. A lot of open questions arise (like commitment schemes for things) since this is
getting closer to the actual blockchain implementation (that we do not have), which is something I'll be working on
addressing in the coming weeks too.

[PR 143]: https://github.com/nazar-pc/abundance/pull/143

## Upcoming plans

With basic execution environment, transactions and transaction pool, we'll need blocks and a chain of blocks, meaning
blockchain. The plan there right now is to use [Subspace protocol reference implementation] as the foundation
initially (that I'm very familiar with). I'll strip all the unnecessary bits there (everything related to domains,
votes, probably most of the reward issuance logic, etc.) first to reduce the number of things to carry around. With
that, we'll not be using the Substrate framework either, so decoupling logic from Substrate and probably converting core
consensus pieces into contracts will be the next logical step after that.

[Subspace protocol reference implementation]: https://github.com/autonomys/subspace

This will be a long-is process since Substrate provides networking stack for the blockchain, which will have to be
replaced and networking stack used for DSN is not quite in the shape to replace that yet. With that, we'll hopefully
have a basic blockchain running a couple of system contracts one to two months later.

Once that is done, we'll be able to compose instances of such blockchains into a sharded hierarchy and experimenting
with how things fit together based on the research Alfonso is doing.

That is my small update for now, see you next time!
