# 🚧 Project Abundance 🚧

[![🚧 Website](https://img.shields.io/badge/🚧_Website-grey)](https://abundance.build/)
[![Rust Docs](https://img.shields.io/badge/Rust_docs-grey?logo=rust)](https://abundance.build/rust-docs)
[![📖 Book](https://img.shields.io/badge/📖_Book-grey)](https://abundance.build/book)
[![💬 Zulip chat](https://img.shields.io/badge/📖_Zulip_chat-grey)](https://abundance.zulipchat.com/)

Researching next-gen blockchain architecture (as of 2026) to achieve ultimate scalability in permissionless setting and
fully resolve Blockchain Trilemma. May or may not succeed but must be fun.

## Status

The current status is heavy WIP with somewhat regular updates on the [website]. Read the [book] for architecture details
and check the code for details otherwise. Most of the things are missing right now, but that'll change over time.

[website]: https://abundance.build/

[book]: https://abundance.build/book

## Useful crates for third-parties

While this repository is focused on blockchain R&D, there are some useful Rust crates that others might find useful too:

* [`ab-aligned-buffer`] - efficient abstraction for memory buffers aligned to 16 bytes (`u128`) with both owned and
  shared variants
* [`ab-blake3`] - `const fn` and GPU-friendly ([rust-gpu]) BLAKE3 primitives
* [`ab-chacha8`] - small GPU-friendly ([rust-gpu]) software implementation of ChaCha8
* [`ab-direct-io-file`] - cross-platform APIs for working with files using direct I/O
* [`ab-io-type`] - infrastructure for zero-cost zero-copy serialization/deserialization
* [`ab-merkle-tree`] - high-performance Merkle Tree and related data structures (Merkle Mountain Range, Sparse Merkle
  Tree) using BLAKE3 (can be generalized to other hash functions if necessary)
* [`ab-riscv-primitives`] - composable RISC-V primitives (instructions, registers) and abstractions around them
* [`ab-riscv-interpreter`] - composable and generic RISC-V interpreter

[`ab-aligned-buffer`]: https://docs.rs/ab-aligned-buffer

[`ab-blake3`]: https://docs.rs/ab-blake3

[`ab-chacha8`]: https://docs.rs/ab-chacha8

[`ab-direct-io-file`]: https://docs.rs/ab-direct-io-file

[`ab-io-type`]: https://docs.rs/ab-io-type

[`ab-merkle-tree`]: https://docs.rs/ab-merkle-tree

[`ab-riscv-primitives`]: https://docs.rs/ab-riscv-primitives

[`ab-riscv-interpreter`]: https://docs.rs/ab-riscv-interpreter

[rust-gpu]: https://github.com/rust-gpu/rust-gpu

Many of these crates are `no_std`, do not require an allocator and make efficient use of stack through const generics.
Where possible, [`no-panic`] is used to guarantee absence of panics at compile time for high reliability and more
efficient code generation. None of the listed crates have any dependencies on anything in this repository that is
specific to this project, so can be used externally with minimal dependencies.

[`no-panic`]: https://docs.rs/no-panic

Note that these crates may use experimental Nightly Rust features to achieve their goals, and as a result, most of them
do not work on stable Rust (yet).
