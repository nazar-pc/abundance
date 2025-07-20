---
title: Sparse Merkle Tree and client database preparation
date: 2025-07-20
draft: false
description: "While the client database implementation is not ready yet, I managed to implement a few underlying components."
tags: [ status-update ]
authors: [ nazar-pc ]
---

After adventures with rust-gpu, which I still monitor periodically, I moved on to the client database implementation,
which is required for proper blockchain operation, and which is one of the bigger undertakings. Unfortunately, the
database as such isn't quite ready yet, but I did some preparation and would like to share some details about the
database architecture.

<!--more-->

## Sparse Merkle Tree

The blockchain state is expected to be organized as a Sparse Merkle Tree, but just like with regular Merkle Tree, none
of the implementations on crates.io looked like a good fit. With that, I decided to implement my own and merged it in
[PR 328].

[PR 328]: https://github.com/nazar-pc/abundance/pull/328

The implementation is quite specific to the use case at hand. For example, it currently only supports up to \\(2^128\\)
leaves since this is how many addresses the blockchain supports. It is not quite as efficient as it could be since it
hashes one pair of leaves at a time, but it already includes an efficient handling of empty/missing leaves. The
improvements in this department will come later, just like Merkle Tree will become much faster with [SIMD-accelerated]
hashing of multiple values. In fact, unofficial `blake3::platform` APIs already make it possible, which I plan to take
advantage of before something like that is upstreamed.

[SIMD-accelerated]: https://github.com/BLAKE3-team/BLAKE3/issues/478

Architecturally, Sparse Merkle Tree implementation is very close to the Merkle Tree implementation that already existed,
but it "hashes" two zero-filled nodes into a zero-filled node, which allows optimizing proof size. Other than that it is
still a recursive data structure, and larger trees can be built from a set of smaller ones.

Implementations I found on crates.io are tied to the storage backend, while what I implemented is basically ephemeral.
It just takes an iterator with leaves and an input and produces a root as an output. It is expected that a bunch of
smaller subtrees will be stored on disk and re-hashing will only need to be done on small parts of it, while
higher-level nodes will be retained in memory. Since an optimized BLAKE3 implementation can hash ~9 GB of data on a
single CPU core on my machine, a small in-memory cache will go a long way.

## Direct file I/O

I learned the hard way while working on the Subspace farmer that the way OSs handle file I/O is quite inefficient when
you know exactly what you're doing. Application-level caching is much more effective and efficient, that is assuming it
is necessary at all. The code for direct I/O was hidden in the farmer implementation. However, now that I want to reuse
it for database implementation, I extracted it into `ab-direct-io-file` crate in [PR 332], then in [PR 333] farmer
started to use it instead of its own copy.

[PR 332]: https://github.com/nazar-pc/abundance/pull/332

[PR 333]: https://github.com/nazar-pc/abundance/pull/333

The implementation should work for now, but eventually I'd like to get back to experiments with async I/O using
`io_uring`, which I tried in the past already, but it didn't perform particularly well. With what I know now, I think it
may have been caused by the fact that back then I was not doing direct I/O, so OS interference had a large impact.

## Client database

Those are both components that will be used for/in conjunction with the client database. The database is not in a state
to open a PR and is not usable yet, but I can share some ideas and design decisions in the meantime.

As I mentioned in [Blockchain as a library], the reference implementation of a node will only support block authoring by
default, which opens a design space for optimizations.

[Blockchain as a library]: ../2025-04-26-blockchain-as-a-library

I'd really like to achieve constant-size disk usage just like on the farmer: node will pre-allocate all the necessary
space upfront and only use that in the runtime.

Another observation is that while the blockchain does have short-term natural forks, most of them are short-lived, so
those that didn't survive for long don't need to be written to disk at all.

Yet another important feature is that the blocks at 100 blocks deep are immutable, and even beyond that there is a
limited number of blocks that need to be stored. So the blocks don't need to be written into a fancy database, a flat
file is just fine. Due to limited overall size, the file can be scanned on startup and mapping from blocks to offsets
can be maintained in memory. In fact blocks in memory can also be stored in a flat list, only a single map from a block
hash to an offset into an in-memory list needs to be stored. This all means very compact and efficient data structures
both in memory and on disk.

Now there is a bit of a challenge with state. It is both larger than blocks, and it is stored permanently. Thankfully,
due to only storing roots of individual accounts rather than the contents of the state as such, the total amount of disk
space used will be incomparably smaller than in more traditional blockchains. So much so that for early stages of the
network, it'll likely all easily fit into in-memory cache, but we still need to prepare for it to grow over time.

The way I'm thinking to approach it is to essentially start with an empty Sparse Merkle Tree, represented by a simple
flat list of leaves, which can be hashed together to get a state root. On updates, a new list can be written to disk.
Writes to SSDs happen in pages anyway, so anything smaller than 4 kiB (on modern SSDs more like 16 kiB) ends up
consuming the whole page of actual disk write resource anyway. Once the list becomes too large, we split it in half by
the key space until it fits into a single page again.

This can be done recursively over time as the state gets bigger, possibly with some "incremental diff" stored for most
blocks on top of earlier checkpoint to trade disk space for computation. In-memory representation will likely store the
most information about the state of the best block, since that is the one, which will be accessed most often. Remember,
this is a block authoring node, not RPC, so we care very little about older state and don't need it in most cases, and
it is fine to pay a bit extra for its access.

In-memory cache will store references to these pages with some number of caches roots of subtrees of the larger state
Sparse Merkle Tree, so the final root can be derived quickly without hashing the whole thing.

Both blocks and state subtrees and other elements are expected to be stored in a single flat file as "storage items."
Each storage item will be aligned to the disk page size for efficient access. Writes will be done in full pages, storage
items will be cleared in bulk, and notification will be sent to the SSD that the pages are no longer used.

To handle interrupted disk writes, a few checksums will be added on storage item: one for header and one for the
contents. The checksum for the header will be stored twice, once at the beginning of the storage item and once at the
end. This should cover the most likely case of incomplete write where the tail of the storage item is missing.

Storage items will also not be allowed to cross certain boundaries, like every multiple of 1 GiB. which will allow to
quickly and efficiently scan the whole file and read its contents with high level of concurrency since read can start at
any boundary and be guaranteed to hit the beginning of a storage item.

Overall this should result in a flat file with a simple structure, next to no write amplification and O(1) reads for
most practical purposes. This can only be done with a specific use case in mind and only due to architectural decisions
around smart contracts design done earlier.

## Upcoming plans

I delayed this update a bit because I wanted to have some early version of the database working first. But it takes a
substantial amount of time and I spent way too much time thinking and researching before I could even write anything.
There was some amount of procrastination involved, of course. But I think the description should give you an idea of
what it will look like, and hopefully it'll not take too much time to have something to show as well.

My plan for the next few weeks is to continue working on the client database. I might make another detour to implement
SIMD-accelerated BLAKE3 hashing for Merkle Tree (probably just balanced to start) and Sparse Merkle Tree to see how
close it can get to the theoretical limits with current design that uses iterators, etc. Also once [const folding PR] is
merged into rust-gpu, I'll likely jump back and merge at least [compute_fn()] implementation, which I already
implemented earlier (except tests).

[const folding PR]: https://github.com/Rust-GPU/rust-gpu/pull/317

[compute_fn()]: https://subspace.github.io/protocol-specs/docs/consensus/proof_of_space#compute_fn

I was also thinking about publishing some of the libraries like `ab-blake3` and `ab-direct-io-file` to crates.io since I
do believe they might be useful to others and there isn't really any library currently published that does what those
libraries do.

Still a lot of work in different areas, so I'll certainly not being blocked completely by anything except myself. If any
of this was interesting, and you'd like to discuss it, ping me on [Zulip]. Otherwise, I'll have another update in a week
or so.

[Zulip]: https://abundance.zulipchat.com/
