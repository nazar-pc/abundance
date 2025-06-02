---
title: Path to block production and procrastination
date: 2025-06-02
draft: false
description: Some work on refactoring for initial block production and some performance improvements
tags: [ status-update ]
authors: [ nazar-pc ]
---

There have not been an update from me last week, how come? Well, it didn't seem like there was anything particularly
substantial to share, mostly some refactoring, so I decided to skip it. This week though I have a few words to say about
the path towards block production (not there yet, but getting closer). Some this is a boring process, I procrastinated
some too, diving into various topics with some interesting performance improvements and new learnings.

<!--more-->

## Sharding architecture

## Block-related changes

In the [previous update] I explained the way block is going to look like, but that was only parsing part of the binary
format, there was no owned data structures to move around, send over the network, etc. Owned version of data structures
landed in [PR 253] with some follow-up changes in [PR 254]. I then looked at reward singing and noticed we didn't
actually benefit from advanced features of Sr25519, so I migrated to Ed25519 to make it easier for third-parties to
build custom farmers and verify blocks, especially in other languages, in [PR 251] and [PR 252]. Funnily enough, we may
need to switch back depending on how we need to modify consensus implementation for sharding, but we'll see when we get
there.

[previous update]: ../2025-05-19-what-does-a-block-look-like/

[PR 253]: https://github.com/nazar-pc/abundance/pull/253

[PR 254]: https://github.com/nazar-pc/abundance/pull/254

[PR 251]: https://github.com/nazar-pc/abundance/pull/251

[PR 252]: https://github.com/nazar-pc/abundance/pull/252

## Refactoring of components and reducing Substrate dependencies

Now to build and verify blocks, proof of time is crucial, but they were still under `subspace` directory. I refactored
and moved `subspace-proof-of-{space,time}` into `crates/shared/ab-proof-of-{space,time}` in [PR 256]. I even made PoT
verification a bit faster by removing heap allocations from it (both `alloc` and `std` features were removed).

[PR 256]: https://github.com/nazar-pc/abundance/pull/256

Then I went on to refactor `sc-subspace-proof-of-time` to make it better suited for separation from Substate
dependencies in [PR 262], eventually extracting `crates/node/ab-client-proof-of-time`, which was very similar to
`sc-subspace-proof-of-time`, except thanks to refactored abstractions I was able to move just local timekeeper, leaving
block import and gossip handlers in `sc-subspace-proof-of-time` for now. This way I can continue work on block
production without implementing networking stack and having the whole block import pipeline working first. Local
timekeeper is all I need to get started, but the API is already in place to bring other components when they are ready.

[PR 262]: https://github.com/nazar-pc/abundance/pull/262

With that done I moved on to massaging `sc-consensus-subspace` in [PR 265], whose slot worker was refactored to
integrate the minimal logic of `sc-consensus-slots` without relying on it anymore. This is a stepping stone for
disentangling it from Substrate completely. More work is needed, but that is basically where I am so far.

[PR 265]: https://github.com/nazar-pc/abundance/pull/265

BTW the node and farmer implementations inherited from Substrate are still functional enough to produce blocks, etc. A
lot of original logic is gone by now of course, but at least I have something to sanity check my changes against.

## Performance improvements

All that is often boring mechanical work or something that feels really vague and unbounded, which is
procrastination-inducing. What that means is that I'm spending time reading about random things and doing experiments
that sometimes lead to unexpected performance improvements.

There are Proof-of-Time and Proof-of-Space components in the protocol design, which are performance sensitive.
Especially since sync and plotting performance will dictate parametrization and to some degree feasibility of sharding
design, they need to be as fast and efficient as possible.

### Proof-of-Time

Since I was working around PoT a bit and already improved its verification performance by ~8-9% by avoiding heap
allocations, I recalled that `aes` crate still [doesn't support VAES], so I decided to implement an optimized version
for PoT with [VAES] specifically myself in [PR 260]. The results are awesome, on Zen 4 CPU (AVX512-capable) the time to
verify PoT reduced ~2x, taking overall 16x less time than proving and ~10x less time than the fastest CPUs out there can
prove:

```
Before:
verify                  time:   [204.92 ms 205.10 ms 205.30 ms]

After:
verify                  time:   [102.38 ms 102.43 ms 102.53 ms]
```

[doesn't support VAES]: https://github.com/RustCrypto/block-ciphers/issues/372

[VAES]: https://en.wikipedia.org/wiki/AVX-512#VAES

[PR 260]: https://github.com/nazar-pc/abundance/pull/260

Then I noticed something strange in CI when working on extending `no-panic` to `ab-proof-of-time` in [PR 264] and later
enabling it on Windows in [PR 268], that I decided to not only implement it for AVX512-capable CPUs, but also for a few
of those that support VAES with AVX2 only (2 blocks at a time rather than 4) and even optimized version for many older
CPUs with just [AES-NI] in [PR 269].

[PR 264]: https://github.com/nazar-pc/abundance/pull/264

[PR 268]: https://github.com/nazar-pc/abundance/pull/268

[AES-NI]: https://en.wikipedia.org/wiki/AES_instruction_set#x86_architecture_processors

[PR 269]: https://github.com/nazar-pc/abundance/pull/269

While in a mood, I ended up implementing optimized version for aarch64 as well in [PR 270], which on [Raspberry PI 5]
SBC allowed to substantially improve performance (my guess is that Apple Silicon CPUs will improve as well, but I do not
own any):

```
Before:
verify                  time:   [1.1665 s 1.1817 s 1.1976 s]

After:
verify                  time:   [835.30 ms 835.30 ms 835.31 ms]
```

[PR 270]: https://github.com/nazar-pc/abundance/pull/270

[Raspberry PI 5]: https://www.raspberrypi.com/products/raspberry-pi-5/

### Proof-of-Space (Chia)

Proof-of-Space was next. Over last 2 years I learned a lot about performance on modern CPUs and how to optimize
Proof-of-Space, but I was still repeatedly trying to improve its performance, often unsuccessfully. Last week though I
did make substantial progress again.

First there was [PR 266], where I managed to optimize code generation to make compiler vectorize more code for me by
structuring it a little differently. As a nice coincidence, the code became more readable too, which is not always the
case when optimizing for performance.

[PR 266]: https://github.com/nazar-pc/abundance/pull/266

```
Before:
chia/table/single       time:   [1.0668 s 1.0756 s 1.0862 s]
chia/table/parallel/8x  time:   [905.01 ms 919.11 ms 933.34 ms]
chia/verification       time:   [11.094 µs 11.533 µs 11.985 µs]

After:
chia/table/single       time:   [1.0245 s 1.0329 s 1.0432 s]
chia/table/parallel/8x  time:   [830.37 ms 843.37 ms 855.47 ms]
chia/verification       time:   [8.9454 µs 8.9612 µs 8.9726 µs]
```

I also was looking at GPU implementation more closely last few weeks and tempted to try to implement Proof-of-Space
table creation on GPU using [CubeCL]. Implementation in Subspace is designed for CUDA and ROCm, but it only supports GTX
16xx+/RTX 20xx+ GPUs on Nvidia side and on AMD side it is quite awkward to use, doesn't work on Windows with consumer
GPUs and requires separate binaries for Nvidia and AMD due to [strange] [design] [decisions] used in underlying
libraries. Not to mention that it doesn't work on AMD iGPUs, doesn't support GPUs from Intel or other vendors (like on
various ARM SBCs).

[CubeCL]: https://github.com/tracel-ai/cubecl

[strange]: https://github.com/supranational/blst/pull/153

[design]: https://github.com/supranational/blst/pull/203

[decisions]: https://github.com/supranational/sppark/commit/0a41eb5eea29975022166969d34550379615f271#r148977505

So going back to GPU implementation, I noticed it follows the Rust CPU implementation quite closely, even copied some
comments from it verbatim. The big unique thing was a custom GPU-optimized sorting implementation. Not only that, it
only sorted `Y` values, not the rest 🤔

Thinking about it some more, for the first Chia table we can indeed use stable sort and only consider `Y` because `X` is
already sorted originally and will remain in deterministic order if stable sort is used for `Y`. Now for the rest of the
tables it turns out a similar logic can be applied, but the buckets need to be processed in deterministic order, which
while not the case in Rust implementation right now, can be done and will open the possibility to sort by only `Y` as
well.

There is no PR with those changes, but I confirmed that substantial gains can be had from that change alone. Not only
that, GPU uses [Radix sort], which is really well suited for sorting integers. I read a bunch about that and looked at
few Rust implementations like [voracious_sort], which bring further gains (though that particular library is actually
not `no_std`-compatible yet). But then while [reading about voracious sort on author's blog], I learned that there is
even more niche sort that is even faster and fits the use case of sorting `Y` (which while are `u32` internally right
now, only store `K` bits of information in them, `K=20` right now in Subspace) called [counting sort], which is very
similar to Radix sort, but much simpler and has a single round!

[Radix sort]: https://en.wikipedia.org/wiki/Radix_sort

[voracious_sort]: https://github.com/lakwet/voracious_sort

[reading about voracious sort on author's blog]: https://axelle.me/2020/11/21/voracious-sort/

[counting sort]: https://en.wikipedia.org/wiki/Counting_sort

Counting sort is something I'm planning to use in Proof-of-Space, and it will work great or GPUs as well. Since the use
case is very narrow and specific, very simpler yet efficient implementation can be written and I have high hopes for it!
It'll also be a stepping stone for me in preparation for writing a GPU implementation.

BTW most of these changes are already backported to Subspace with more coming soon.

## Upcoming plans

So that is what I've been busy with last couple of weeks. Planning to continue working on block production in the near
future and will probably tackle that sorting replacement adventure, maybe even dabble into GPU programming (was
successful at avoiding it so far).

If you have any questions or thoughts, [Zulip] chat is currently the best place for reaching out. See you next week with
a fresh portion of updates.

[Zulip]: https://abundance.zulipchat.com/
