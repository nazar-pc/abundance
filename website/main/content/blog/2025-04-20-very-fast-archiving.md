---
title: Very fast archiving
date: 2025-04-20
draft: false
description: Looking into vector commitments and other updates
tags: [ status-update ]
authors: [ nazar-pc ]
---

Last time I mentioned that I was looking into Merkle Trees to replace KZG. This week it happened, the whole codebase is
basically free from KZG. The only place that is not fully fixed and where I am looking for help is [GPU plotting], it
broke with all these changes and isn't the highest priority for me to fix right now.

[GPU plotting]: /book/Contribute.html#gpu-plotting

<!--more-->

## Replacing KZG

KZG was used for vector commitments in a few places, including archiving and plotting.

For archiving, it was easy: we have records with chunks to commit to, then we take commitments of all roots and create
another commitment over them and include corresponding witnesses/proofs in each piece, so they can be verified against
global history. Works basically the same way as before, but the proof is now larger than before.

For plotting, it turned out to be a bit more tricky. There is a homomorphic property that allowed farmer to extend the
polynomial created over record chunks to get some parity chunks (erasure coding), but still being able to generate
proofs for parity chunks that verify against original record commitment. With Merkle Trees that is no longer the case,
but with both Merkle Tree creation and erasure coding (see below) being so much cheaper, we can erasure code record
chunks during archiving and commit to them too. This is still fast, and when the farmer is redoing the same erasure
coding later, they can generate proofs that successfully verify against record commitment. Problem solved!

[PR 175] is where KZG was completely replaced with Merkle Trees, but a lot of TODOs remained in the code for further
cleanups. This alone more than doubled the archiver performance, while doing 2x erasure coding and commitments than
before!

[PR 175]: https://github.com/nazar-pc/abundance/pull/175

[PR 180] also added parity chunks directly to the piece, such that when doing piece verification, it isn't necessary to
do erasure coding (and heap allocations as the result). In fact, after some more changes and refactoring, [PR 187]
finally made it possible to use `subspace-verifiction` in `no_std` environment with no heap allocations.

[PR 180]: https://github.com/nazar-pc/abundance/pull/180

[PR 187]: https://github.com/nazar-pc/abundance/pull/187

## Replacing BLS12-381 erasure coding

With KZG gone, we were still doing erasure coding using BLS12-381, but we don't have to!

After looking at various options, I ended up picking [reed-solomon-simd], which appears to be the fastest readily
available library in Rust and has a reasonably nice public API. Unfortunately, it doesn't yet work in `no_std`
environment, so I sent a corresponding [PR implementing that].

[reed-solomon-simd]: https://github.com/AndersTrier/reed-solomon-simd

[PR implementing that]: https://github.com/AndersTrier/reed-solomon-simd/pull/63

Erasure coding was swapped from using [grandinetech/rust-kzg] in [PR 181], which again more than quadrupled archiving
performance, wild!

[grandinetech/rust-kzg]: https://github.com/grandinetech/rust-kzg

[PR 181]: https://github.com/nazar-pc/abundance/pull/181

## Further archiver improvements

The usage of BLS21-381 meant we could only operate on 254-bit values, which in practice means 31-byte chunks for
efficiency and simplicity purposes. This resulted in a lot of boilerplate across the codebase, introduction of
`RawRecord` vs normal `Record` (first contains a multiple of 31-byte chunks vs 32-byte chunks of normal record). This
was also very annoying for data retrieval since even within a single record, it wasn't possible to simply slice the
bytes. It was necessary to chunk the record into 32 byte chunks and then throw each 32nd byte away, what a mess!

Now that BLS12-381 was completely gone from both commitments and erasure coding, it was possible to end this.

First, [PR 182] unified chunk size to be 32 bytes. This simplified the code a lot across the board.

[PR 182]: https://github.com/nazar-pc/abundance/pull/182

Another issue that plagued the codebase for a long time was source/parity pieces/records interleaving. This is how
polynomial extension worked in [grandinetech/rust-kzg,] and we went with it as a natural behavior, but it was another
source of complexity and inefficiency, especially in data retrieval. [PR 184] ended that too, streamlining the code even
further.

[PR 184]: https://github.com/nazar-pc/abundance/pull/184

Finally, with refactoring in [PR 185] and [PR 187], the performance improved even further, while code was even easier
to read.

[PR 185]: https://github.com/nazar-pc/abundance/pull/185

[PR 187]: https://github.com/nazar-pc/abundance/pull/187

## The results and lessons learned

So what are the results? Multithreaded archiving time decreased from ~4.5 seconds per segment to just 0.24 seconds.
This is **~18x performance improvement** 🎉. In fact, single-threaded archiving at 1.8 seconds is now faster than
multithreaded before, imagine that! And there are still opportunities for performance optimizations left to explore if
it ever becomes critical. This also implies much faster piece reconstruction in case it is necessary due to node sync
or plotting, this was a very costly operation in the past.

Moreover, I originally thought that the logarithmic proof size would increase the piece overhead. It turns out we're
saving a lot more by not wasting each 32nd byte of the record on padding due to KZG, so there is actually a large
utilization improvement! It will actually be very handy as out proofs increase in size when we add sharding to the mix.
The only place where it does use more space is the solution data structure. While unfortunate, being one per block it is
not too bad.

When designing Subspace we had many iterations of the design, and I think at some point we were stuck with KZG without
necessarily fully taking advantage of it. There were various ideas about things like distributed archiving, which might
end up benefiting from KZG, but at this point I don't really think it is worth it. It is too slow, cumbersome to use and
not post-quantum secure.

In retrospective, all the changes described here could be applied to Subspace if we looked back at things from the first
principles after we have designed the latest version of the protocol.

## Future improvements

Funnily enough, these are not all the breaking changes I want to do to the codebase. [issue 183] describes the remaining
known data retrieval challenges only discovered after mainnet launch (we didn't really try to retrieve a lot of data
during testnets) and should be addressed to greatly simplify the logic. Once done, retrieval will be much more efficient
and easier to reason about (current code in Subspace is very convoluted, but unfortunately it has to be to maintain
efficiency, naive retrieval is much more wasteful).

[issue 183]: https://github.com/nazar-pc/abundance/issues/183

## Upcoming plans

So what is next? Well, with a bunch of technical debt dealt with, I'll probably try to put some of the consensus logic
into system contracts. All system contracts have so far managed to avoid heap allocations and making
`subspace-verification` work without any was a stepping stone towards using it in contracts too.

In no so far future I'd like to have a simple single-node "blockchain" that produces blocks, while not being based on
[Substrate] in any way. It'll take time to get there, but progress is being made towards that.

[Substrate]: https://github.com/paritytech/polkadot-sdk/tree/master/substrate
