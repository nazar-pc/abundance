---
title: First crates on crates.io
date: 2026-03-14
draft: false
description: Some crates are now available on crates.io for general use
tags: [ announcement ]
authors: [ nazar-pc ]
---

The first batch of crates is now available on [crates.io]!

[crates.io]: https://crates.io/users/nazar-pc

I've been working on a lot of stuff during the last year, but being buried deep in the repository, the code is not easy
to discover and reuse. I picked the first batch of crates that should be useful for the broader public and published
them on crates.io with the hope to attract some users and contributors.

All of these crates are completely independent and do not have dependencies on anything blockchain-specific in this
repository. Most of them are `no_std` and do not even require an allocator.

But wait, there is more! In many cases APIs are guaranteed to never ever panic (at compile time!). For example, Merkle
Tree construction and verification have those guarantees. This is awesome for high reliability and compiler
optimizations!

A short "advertisement" for each of the crates follows below, all these crates are also now mentioned in the main readme
in the repository.

Note that most crates leverage Nightly Rust features heavily and may be unusable on Stable Rust (for now).

<!--more-->

## [`ab-aligned-buffer`]

[`ab-aligned-buffer`]: https://docs.rs/ab-aligned-buffer

This crate provides `OwnedAlignedBuffer` and `SharedAlignedBuffer` data structures that wrap heap-allocated bytes
aligned to 16 bytes (`u128`). The first allows mutations, second allows cheaper than `Arc` clones.

The key use case is to store correctly aligned data structures, allowing to cast allocated bytes to data structures and
back. Check `ab-io-type` crate below for more details, these two are often used together. In this context, it is also
often paired with [`yoke`] crate, such that a wrapper data structure can combine the backing buffer and a reference to
its representation as a data structure.

[`yoke`]: https://docs.rs/yoke

This is how owned block headers and bodies are implemented, for example, just a bunch of bytes in an aligned buffer,
which can be directly used as a data structure. Of course, in the case of a buffer received from the network, the
contents need to be read and validated. However, when verified data is read from the disk, the buffer can be used as a
data structure with barely any work at all.

Example:

```rust
/// An owned version of [`BeaconChainBody`].
///
/// It is correctly aligned in memory and well suited for sending and receiving over the network
/// efficiently or storing in memory or on disk.
#[derive(Debug, Clone)]
pub struct OwnedBeaconChainBody {
    inner: Arc<Yoke<BeaconChainBody<'static>, SharedAlignedBuffer>>,
}

impl GenericOwnedBlockBody for OwnedBeaconChainBody {
    // ...

    type Body<'a> = BeaconChainBody<'a>;

    // ...

    #[inline(always)]
    fn body(&self) -> &Self::Body<'_> {
        self.inner.get()
    }
}

```

The reason for aligning to only 16 bytes is that `ab-io-type`'s types are aligned to at most 16 bytes, and using higher
alignment would waste more memory than it should.

## [`ab-blake3`]

[`ab-blake3`]: https://docs.rs/ab-blake3

`const fn` and GPU-friendly ([rust-gpu]) BLAKE3 primitives. Somewhat exotic, but it is nice to be able to embed a
constant into the binary, which itself is a hash of some value.

[rust-gpu]: https://github.com/rust-gpu/rust-gpu

For example, it is used to embed the key for keyed hashing of internal nodes in a Merkle Tree:

```rust
/// Used as a key in keyed blake3 hash for inner nodes of Merkle Trees.
///
/// This value is a blake3 hash of a string `merkle-tree-inner-node`.
pub const INNER_NODE_DOMAIN_SEPARATOR: [u8; KEY_LEN] =
    ab_blake3::const_hash(b"merkle-tree-inner-node");
```

It also leverages private APIs of the [`blake3`] crate to offer a few helper methods that allow better code gen when
hashing at most a single block (64 bytes) or a single chunk (1024 bytes) of data. It also has a crucial function for
hashing multiple exactly block-sized values, which massively accelerates Merkle Tree construction. I used it
[to speed up archiving].

[`blake3`]: https://docs.rs/blake3

[to speed up archiving]: ../2025-08-08-async-transaction-processing/#other-updates

And lastly there is `single_block_hash_portable_words` function that works with `u32` words instead of bytes, which
allows hashing up to a block of bytes on a GPU with [rust-gpu] despite its numerous limitations at the moment.

## [`ab-chacha8`]

[`ab-chacha8`]: https://docs.rs/ab-chacha8

A very basic crate, offers tiny abstraction for ChaCha8 stream cipher that works with `u32` words and works on the GPU
with [rust-gpu] too. They both were used to implement [GPU plotting].

[GPU plotting]: ../2025-10-23-gpu-plotting-works

## [`ab-direct-io-file`]

[`ab-direct-io-file`]: https://docs.rs/ab-direct-io-file

Working with Direct/unbuffered/uncached I/O is kind of painful, especially in a cross-platform way. I spent some time
learning my way around it and extracted a wrapper abstraction that makes life so much easier.

Essentially, it exposes both APIs to write whole 4096-byte pages in a typed manner and higher-level APIs for
reading and writing data at arbitrary offsets, while taking care of read/modify/write low-level details of partial page
reads/writes. I hope you find it useful.

## [`ab-io-type`]

[`ab-io-type`]: https://docs.rs/ab-io-type

For smart contracts I needed a way to do FFI between host and guest code, while also being able to make sense of the
data in a machine-readable way. I also wanted it to be zero-copy. After exploring every available option, I found none
of them quite doing what I need.

With `ab-io-type` you'll get two traits `TrivialType` and `IoType`, which both also have an associated constant with
metadata about the type.

`TrivialType` is implemented for a bunch of primitive types and can be derived on custom types well. Its implementation
means that the type is just a trivial bunch of bytes, so serialization and deserialization are just casting its memory
to/from bytes. No uninitialized bytes (padding, niche bits, etc.) are allowed. Essentially, you get serialization and
deserialization that cost literally nothing, nice!

`IoType` is a bit more advanced and allows optional and variable-length lists of values.

Together, these allow interpreting correctly aligned bytes as complex data structures, while not paying any cost for
serialization or deserialization. This is why they are often backed by buffers from `ab-aligned-buffer` crate.

Not only that, the traits, as mentioned above, have `METADATA` associated constant. It contains a compact binary
representation of the type and allows reading the data field by field if necessary, comparing two data structures for
compatible interface, etc. This is nice for smart contracts since their APIs can be rendered in a somewhat
human-readable way. You can think about it like a schema.

The best thing is, the macro fully auto-generates metadata at compile time! You can use it in `const fn` and similar
contexts, including embedding in static variables of your binary, which is leveraged by smart contracts to make compiled
binaries self-descriptive straight out of the compiler with no post-processing needed!

I'm quite proud of what I have achieved with this crate, I hope it'll see some wider adoption.

## [`ab-merkle-tree`]

[`ab-merkle-tree`]: https://docs.rs/ab-merkle-tree

If you need a VERY FAST and efficient balanced/unbalanced Merkle Tree, Merkle Mountain Range, or Sparse Merkle Tree
implementation that works in `no_std` without any heap allocations whatsoever, this might just be the crate for you.

It is currently tied to BLAKE3 hash function and leverages its strengths but could be generalized to any hash function
if there is a demand for it.

The way most crates build trees is crazy inefficient with lots of allocations, high memory usage, and not benefiting
from SIMD capabilities of BLAKE3 for hashing multiple pairs of leaves. `ab-merkle-tree` changes everything!

It uses const generics of Nightly Rust very heavily to allocate just a large enough array on the stack for the given
data set to store intermediate values when building the tree, computing the root very quickly and efficiently.

Balanced and unbalanced trees have slightly different APIs with different tradeoffs, and together with Merkle Mountain
Range produce the same exact output for identical inputs, essentially making them a subset or generalization of each
other.

The crate doesn't concern itself with disk I/O or other unrelated features that existing crates on crates.io have grown
to contain for some reason.

Give it a try, you might be impressed!

## [`ab-riscv-primitives`]

[`ab-riscv-primitives`]: https://docs.rs/ab-riscv-primitives

This crate defines RISC-V instructions, registers, and abstractions around them. It also allows decoding instructions
into an enum and [supports composition] of base ISA and various instructions for better performance and flexibility. It
currently focuses on RV64 with extensions Zba, Zbb, Zbc, Zbs, B, Zmmul, M, Zkbc, and Zhn. Any permutation of those can
be used with both RV64I and RV64E base ISA through generics.

[supports composition]: ../2026-01-24-making-risc-v-interpreter-faster

Zicsr and Zve64x extensions for RV64 are also present and can be decoded, but more work is needed to make them usable in
the interpreter (see `ab-riscv-interpreter` crate below).

RV32 is currently missing, but I plan to add it and at least M/Zmmul extensions to make it more appealing to the broader
audience, despite me not having any immediate need for it.

I was surprised a crate like this didn't already exist, but at least it does now. Consider contributing more extensions
if you need them.

## [`ab-riscv-interpreter`]

[`ab-riscv-interpreter`]: https://docs.rs/ab-riscv-interpreter

This is a complementary crate to `ab-riscv-primitives` that implements a generic RISC-V interpreter. It is generic over
memory, registers, instructions, syscalls handling... Basically everything you might want can be customized. And it
supports the same composition as the instruction definitions above, so you can pick and choose extensions at will,
check if you benefit from the reduced number of general purpose extensions, all very easily within minutes.

It currently supports everything that `ab-riscv-primitives` does except for Zicsr and Zve64x extensions (I plan to work
on them soon).

The interpreter is not opinionated, it doesn't understand ELF or anything of that kind. You just give it instructions,
set up memory, registers and program counter, and it goes brrr.

There are tests for both the instruction decoder, interpreter, and even many tests from the official [RISC-V test suite]
are running in CI to ensure a decent quality of the implementation, though more audits and tests are always welcome.

[RISC-V test suite]: https://github.com/riscv/riscv-arch-test

## What next?

I'll try to publish more crates on crates.io over time as I find things that should be useful for the broader public. I
also plan to publish updates to existing crates as I work on them more.

Feel free to open issues [on GitHub], ask questions on [Zulip] and consider contributing to the project if some of what
you see overlaps with your use cases.

[on GitHub]: https://github.com/nazar-pc/abundance

[Zulip]: https://abundance.zulipchat.com/
