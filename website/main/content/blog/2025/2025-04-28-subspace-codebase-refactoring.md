---
title: Subspace codebase refactoring
date: 2025-04-28
draft: false
description: Cleanups and preparation for repurposing Subspace components
tags: [ status-update ]
authors: [ nazar-pc ]
---

The last week was lighter on major changes, but there was a lot of cleanups and refactoring done to prepare Subspace
components reuse for building a new blockchain from scratch. Also some improvements based on new developer feedback.

<!--more-->

## Subspace refactoring

This has been happening on and off for some time, but I'm at the point when the pieces of code from Subspace need to be
moved into Substrate-independent new part of the codebase.

Since we're building a substantially different blockchain, some assumptions made in Subspace are not directly applicable
and had to change. The good thing is, it can be done by modifying code in place and keeping Substrate-based node
operational in the meantime.

For example, I mentioned in previous updates the aim to make blockchain fundamentally post-quantum secure. This means
the core of the protocol should not be locked into something that would not be post-quantum secure, while at the same
time elliptic curve crypto today is way more efficient than any PQC schemes. One place conflicting with this was the use
of the public keys in the solution. Interestingly, nothing in consensus really cares about the public key. Instead,
public key hash was used to create plots on the farmer and verifying solution on the node, the only place where the
actual public key was needed is to verify block signature. [PR 200] recognized this fact and replaced public key with
its hash in `Solution` data structure, opening the possibility to hash any kind of public key without changes to this
fundamental data structure. The block signature verification is aware of the public key, of course, and will have to be
adjusted once more signature schemes (including PQC) are introduced, but that is a very narrow scope that is easier to
handle.

[PR 200]: https://github.com/nazar-pc/abundance/pull/200

[PR 204] finally added test to modernized erasure coding implementation and after further tweaks moved it under
`crates` (rather than `subspace`). It also introduced a bunch of new types that add a lot of type safety in places where
type aliases were used before. Together with [PR 199], this allows adding helper methods on many data structures, which
were previously standalone methods. For example, it is now possible to call `solution.verify()` instead of
`verify_solution(&solution, ...)`, which I think is a bit more elegant to write and easier to read.

[PR 204]: https://github.com/nazar-pc/abundance/pull/204

[PR 199]: https://github.com/nazar-pc/abundance/pull/199


Block was always `u32` in Subspace codebase, but for long-running blockchain that may end up using smaller block times
it feels too small, so [PR 207] changed it to `u64`, discovering places where type aliases were misused. Similarly,
Subspace is based on Substrate and expected that pallets would be able to provide object mapping logic for their
transactions, but that seems at odds with high-performance blockchain and general [Blockchain as a library]
architecture. This is why [PR 208] removed object mapping logic from the runtime, expecting that application-specific
object mapping will be done by developers using APIs that the native blockchain will expose to developers off-chain.

[PR 207]: https://github.com/nazar-pc/abundance/pull/207

[Blockchain as a library]: ../2025-04-26-blockchain-as-a-library

[PR 208]: https://github.com/nazar-pc/abundance/pull/208

Finally, there were updates to the document describing the difference from Subspace implementation, which for now is the
closest thing we have to a specification. Notably, [PR 196] renamed all commitments/witnesses to roots/proofs now that
KZG is no longer used and was replaced with Merkle Trees.

[PR 196]: https://github.com/nazar-pc/abundance/pull/196

## New APIs and developer feedback

[Serge] reached out to me recently and was kind enough to try building some contracts, for which I prepared a couple of
handy data structures in [PR 203] and then implemented some fixes and improvements in [PR 209], [PR 210] and [PR 211].

[Serge]: https://github.com/isSerge

[PR 203]: https://github.com/nazar-pc/abundance/pull/203

[PR 209]: https://github.com/nazar-pc/abundance/pull/209

[PR 210]: https://github.com/nazar-pc/abundance/pull/210

[PR 211]: https://github.com/nazar-pc/abundance/pull/211

The feedback was very detailed and extremely helpful, I was only half-joking when I told him that he is the first
developer on a blockchain that doesn't even exist. I hope his adventure will end up in more improvements and additional
example contracts in the repository for others to learn from.

## Upcoming plans

Now with a lot of refactoring complete, I'll be moving more code from `subspace` directory as it is in good enough state
and start designing many consensus-related system contracts. I'll _try_ to somehow use them in Subspace codebase still,
so I can keep some version of the node working end-to-end at all times, but not yet sure how feasible it really is.

If any of this is interesting to you at all, feel free to join our [Zulip] and let's keep the conversation going there.

[Zulip]: https://abundance.zulipchat.com/
