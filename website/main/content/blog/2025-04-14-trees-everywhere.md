---
title: Trees everywhere
date: 2025-04-14
draft: false
description: Looking into vector commitments and other updates
tags: [ status-update ]
authors: [ nazar-pc ]
---

Last week was lighter on code changes and more heavy on research. Specifically, I've been looking into commitment
schemes generally and Blake3 hash function in particular, which was already used in the codebase, but turns out can be
applied in more interesting ways than just a hash function.

<!--more-->

## KZG and quantum computers

There are several places in blockchains where vector commitments are used, and there are several commitment schemes
that might be applicable depending on the use case. [Subspace] being Proof-of-Archival-Storage consensus, required a way
to commit to the archival history of the blockchain among other things, where [KZG] commitments were used.

[Subspace]: https://subspace.github.io/protocol-specs/docs/protocol_specifications

[KZG]: https://iacr.org/archive/asiacrypt2010/6477178/6477178.pdf

KZG feels magical: it allows committing to data set with both commitment and a witness being fixed size (48 bytes)
rather than logarithmic proof when [Merkle Tree] is used. Another neat thing is homomorphic property, which makes
erasure coding of commitments equal to commitment of erasure coded data. This is unfortunately not free as KZG both
require a trusted setup (the Subspace team participated in and used parameters from Ethereum's [KZG Summoning Ceremony])
as well as higher compute cost when committing to data set and generating witness. The drawbacks are unfortunate but
manageable.

[Merkle Tree]: https://wikipedia.org/wiki/Merkle_tree

[KZG Summoning Ceremony]: https://ceremony.ethereum.org/

Recently, however, I've been thinking about future-proofing the design and one of the questions that popped-up is
cryptography resistance to [quantum computers]. And sadly, KZG is not resistant to it, just like a lot of cryptography
used for digital signatures (for example, when singing blockchain transactions).

[quantum computers]: https://en.wikipedia.org/wiki/Quantum_computing

As a result, I've been looking into alternatives. While there are some post-quantum schemes out there, they are not as
optimized and well studied, also none of them have compact constant size commitment and proofs like in KZG. Not only
that, they don't even remotely approach proofs of Merkle Trees, the proofs are simply huge for the use case of archiving
and plotting.

So the conclusion is, we'll have to use Merkle Trees as a reliable, well studied and high-performance alternative.

This issue of quantum computers is the reason why I removed incremental archiving support in [PR 168] (workaround that
amortizes slow KZG commitment creation). With Merkle Trees the complexity of incremental archiving is unnecessary, so we
can simplify the code a bit.

[PR 168]: https://github.com/nazar-pc/abundance/pull/168

The topic of quantum computers will surface a few more times in the future. In the past, transaction handling was
already described in a way that is agnostic to the cryptography used for transaction signing. This is also part of the
reason why reward address (tied to Sr25519 signature scheme) was removed from `Solution` data structure in [PR 169].

[PR 169]: https://github.com/nazar-pc/abundance/pull/169

## Blake3

[Blake3] is a modern and very fast hash function. The reason it is fast is not only because the underlying primitives
are fast, but also because it is actually based on Merkle Tree internally, rather than more common (in hash functions)
[Merkle-Damgård construction]. Its design allows for both instruction-level and multi-threading parallelism.

[Blake3]: https://github.com/BLAKE3-team/BLAKE3

[Merkle-Damgård construction]: https://en.wikipedia.org/wiki/Merkle-Damg%C3%A5rd_construction

Most use cases probably use it as a regular hash function due to its speed, but it can also be used in a more advanced
way as an actual tree! In fact, [Bao] project uses it for verified streaming, meaning it can verify downloaded contents
as it is being downloaded without waiting for the whole file to be downloaded before hash can eb checked. Bao was
originally based on already fast [blake2] served as a prototype for blake3 initially, while these days it is rebased on
blake3. The neat thing is that the hash of the file in Bao is the same as if the file was simply hashed with blake3!

[bao]: https://github.com/oconnor663/bao

[blake2]: https://www.blake2.net/

So blake3 has a Merkle Tree internally, and we need Merkle Tree to commit to some data. Does this mean we can use blake3
directly instead of building a custom Merkle Tree and use blake3 as a regular hash? Turns out it is not trivial, but
yes! And not only that, the cost to create such a tree is basically the hashing of the data, very nice!

Blake3 is already used in the codebase as it was used in [Subspace reference implementation] in many places.

[Subspace reference implementation]: https://github.com/autonomys/subspace

Exposing such Merkle Tree will require some low-level access to blake3 primitives and non-trivial logic, so that is
delayed for now. However, [PR 171] already introduced the first (of likely multiple) Merkle Tree implementation that
will be upgraded to take advantage of blake3 properties later.

[PR 171]: https://github.com/nazar-pc/abundance/pull/171

## Other updates

There were other minor updates, but I want to mention just two.

A book now has [Contribute] page with a few topics that we are not actively working on, but would like to see
contributions or collaborate on. Please join out [Zulip chat] to discuss if any of them are interesting, or you have
something else of interest in mind.

[Contribute]: /book/Contribute.html

[Zulip chat]: https://abundance.zulipchat.com/

A second update is that [PR 172] introduced a document that attempts to describe the difference with [Subspace]
specification until the protocol has its own spec.

[PR 172]: https://github.com/nazar-pc/abundance/pull/172

## Upcoming plans

The immediate next step is probably to swap the KZG commitment scheme with Merkle Tree in the archiving. There was some
awkwardness in the implementation due to KZG using 254-bit scalars (or field elements or whatever they are called
there), resulting in hopefully redundant abstractions. Though erasure coding will have to be replaced with a different
implementation though because it is based on the same BLS12-381 curve right now.

We'll see what the future holds for us soon enough, and I'll make sure to post an update about that. See you next week! 
