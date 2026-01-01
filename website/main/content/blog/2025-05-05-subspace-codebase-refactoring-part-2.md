---
title: Subspace codebase refactoring (part 2)
date: 2025-05-05
draft: false
description: More Merkle Trees, archiving improvements and more
tags: [ status-update ]
authors: [ nazar-pc ]
---

This week was very similar to the last one with a bunch of refactoring in cleanups. There were important archiver
improvements/fixes (depends on point of view) and more work on Merkle Trees. Two more crucial crates were moved from
`subspace` to `crates`.

<!--more-->

## Unbalanced Merkle Tree

In previous updates, I mentioned implementation of the balanced Merkle Tree, which was important to get done for KZG
removal. That was helpful at the time, but there are many places in the protocol where number of elements will not be a
power of 2, for example, root of transactions in a block.

I spent a few days working on an efficient implementation that uses minimal amount of stack-allocated memory and made
sure to make it compatible with the existing balanced Merkle Tree. It is compatible in a sense that for the number of
leaves that is a power of 2, the root and proofs are identical. Moreover, the way the root is constructed is actually
the same as in Merkle Mountain Range, so once we introduce MMR, it will be a special case of the unbalanced Merkle Tree.
Essentially, all three will be a generalization of each other, using the same mechanisms and producing the same roots
and proofs for the same input.

The implementation landed in [PR 216] in the simplest case. In the future, all variants can be further optimized with
both SIMD and (if necessary) parallelism. For example, a large tree can be built in parallel, we can split a large
unbalanced tree into a bunch of smaller balanced subtrees, process them in parallel, recombine and repeat until a single
element is left, tree root.

[PR 216]: https://github.com/nazar-pc/abundance/pull/216

The process was a bit frustrating at times, but I'm quite happy with the result and performance, and kind of excited
that it can be improved further with SIMD by A LOT.

## Refactoring core components

[PR 217] finally introduced new type for `BlockNumber`, [PR 218] added the same for `BlockHash` and `BlockWeight`. These
were a long-standing wish of mine, happy to have them now. With those I finally moved `subspace-core-primitives` as
`crates/shared/ab-core-primitives` in [PR 219].

[PR 217]: https://github.com/nazar-pc/abundance/pull/217

[PR 218]: https://github.com/nazar-pc/abundance/pull/218

[PR 219]: https://github.com/nazar-pc/abundance/pull/219

Now that there is `ab-core-primitives`, it was time to integrate some of the duplicated data structures and type aliases
of `ab-*` crates. In [PR 221] `ab-transaction` crate became `transaction` module of `ab-core-primitives` and a few
generic types were either moved from `ab-contracts-common` to `ab-core-primitives` or replaced with those from it.

[PR 221]: https://github.com/nazar-pc/abundance/pull/221

## Archiver

I mentioned some rounds of archiver updates in the past and that I was not done. Yes, it was much faster already and
easier to reconstruct data from, but some major issues with data retrieval [remained].

[remained]: https://github.com/nazar-pc/abundance/issues/183

Remaining problems ended up originating from encoding complexity. Specifically, SCALE
codec's [compact length encoding of vectors]. While it makes a lot of sense as a general purpose encoding feature, it
does cause problems with non-determinism of the total encoding length. For example, imagine there are a few more bytes
left at the end of a segment. Variable length encoding for numbers meant that sometimes increasing segment item by a
single byte results in compact length encoding growing from 1 to 2 or from two to four bytes. So adding one byte of
information may mean actually adding two or even three bytes to the encoding, which may no longer fit into the segment.

[compact length encoding of vectors]: https://docs.polkadot.com/polkadot-protocol/basics/data-encoding/#data-types

Handling this was not straightforward and required careful test cases to make sure implementation doesn't regress. It
also meant that during data retrieval it wasn't possible to know how many bytes of padding there are for certain without
decoding the whole segment, which is very inefficient. I suggested [both mildly horrifying and impressive ™] hack for
this problem that Teor [courageously implemented]. As you can see, implementation wasn't pretty because the problem
isn't to begin with.

[both mildly horrifying and impressive ™]: https://github.com/autonomys/subspace/issues/3318#issuecomment-2552410265

[courageously implemented]: https://github.com/autonomys/subspace/pull/3362

I started with some data structure cleanups in [PR 223], the important outcome of which is that `SegmentHeader` is now
constant size data structure and not an enum, it also implements `TrivialType` now and in-memory representation now
happens to be identical to SCALE codec, nice! In [PR 226] I found the courage myself to get rid of compact length
encoding and replace all lengths with little-endian `u32` representation. These two PRs together make data retrieval a
breeze: just by knowing the length of the data (either because it was stored somewhere or after retrieval of the first
piece) it is possible to know exactly how many pieces are left to download (ideally concurrently) and no extra hashing
trickery or other extra work is needed. This will be an exercise for another time, though. I left object fetcher in a
broken state for now.

[PR 223]: https://github.com/nazar-pc/abundance/pull/223

[PR 226]: https://github.com/nazar-pc/abundance/pull/226

With these changes, I felt it was time to move `subspace-archiver` as `crates/shared/ab-archiver` in [PR 229].

[PR 229]: https://github.com/nazar-pc/abundance/pull/229

## Upcoming plans

Now that the primitives are coming together at `crates`, I'll probably try to move some consensus logic from
`pallet-subspace` to a new system contract, which will be the foundation for building an actual blockchain. Client
(node) side will require more infrastructure and iterations due to how tightly it is integrated with Substrate, so one
step at a time.

That is all I have to share at this point, please join [Zulip] if you have anything to discuss. We have a whopping eight
members there now! 🤯

[Zulip]: https://abundance.zulipchat.com/
