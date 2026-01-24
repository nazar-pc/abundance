---
title: Faster Proof-of-Space (part 2)
date: 2025-09-07
draft: false
description: Second part in a series of Proof-of-Space optimizations
tags: [ status-update ]
authors: [ nazar-pc ]
---

{{< katex >}}

In the [part 1] I shared some background information, performance improvements and future opportunities. Since then, I
was pursuing various approaches. Some worked out nicely, others were not so fruitful. Overall, I have achieved a
substantial performance improvement on CPU with a few more options still remaining on the table, all while becoming
substantially more GPU-friendly.

[part 1]: ../2025-08-26-faster-proof-of-space-part-1

<!--more-->

## Specification

Reading upstream Chia documentation to understand how things should be implemented is challenging, so there is a
[Subspace specification] that describes things in a much clearer and more approachable way. Well, at least in theory it
does that. In practice, though, it confuses the specification of what needs to be done with an efficient implementation
strategy. And at least for our use cases, it just happened to be the case that the implementation strategy assumed there
wasn't the best one.

[Subspace specification]: https://subspace.github.io/protocol-specs/docs/consensus/proof_of_space

What we're essentially doing is creating a bunch of tables and searching for matches between them. Yes, we can sort the
tables, but that is, strictly speaking, not necessary. The only thing we actually need is buckets that represent ranges
of `y` values, but the order inside doesn't really matter and never surfaces in the proof structure. The only thing
affected is proof order, which has already been a bit of an annoyance before.

It is easier to create a sequential implementation that is deterministic, it is a bit harder to make one that is fast.
And it is even more difficult to create a fast parallel one that behaves the same way on both CPU and GPU.

## New high-level strategy

I started with the observation that only buckets are really necessary. I already did some statistical analysis mentioned
in the previous post to discover that for any K that is relevant, the upper bound for the size of the bucket is 512.
Coincidentally, the number of matches for a pair of buckets also has the same upper bound.

With this knowledge we can do two major implementation changes:

* instead of sorting tables by `y` value, only assign `y` values to buckets (by dividing them by `PARAM_BC = 15113`)
* when searching for matches in pairs of buckets, store the results in the pre-allocated bucket-sized allocation

This results in a few things that are good for performance:

* results of matches no longer need to be concatenated and sorted, they can stay where they were written originally when
  the match was found and parallel version can easily achieve the same deterministic order as sequential one
* instead of doing a bunch of memory copies and potential allocations (depending on the sorting algorithm), we do a
  single scan to assign buckets to `y` values

While bucketing requires a lot of random writes, the small total size of the data appears to be quite friendly for CPU
caches and performs reasonably well.

I even tried to use SIMD for bucketing, but faced a problem, which was one of the promising potential optimizations I
mentioned last time.

## Handling of `y` duplicates and size bounds

One of the challenges with SIMD bucketing is the fact that `y` values can be duplicated. The behavior of the scatter
operation is that only the last write will be observed. On the surface this is not a problem, but it does impact the
number of matches found down the line and the number of proofs that can be found as the result. After doing experiments,
it turned out that this decreases the number of proofs found too much. By too much I mean that it would not be possible
to make plotted sector contain only the chunks which can be farmed, which is undesirable.

If duplicates are no-go, how many matches do we actually need and how small can we make the buckets? I did many more
experiments and came up with numbers 288 (a multiple of 32) and 272 (a multiple of 8) for both. These appear to be large
enough for plenty of proofs to be available, while also being substantially smaller than 512, which reduces memory usage
and significantly increases performance. These smaller sizes should also make it more likely that the data will fit in
shared memory on more (all?) GPUs.

It is really important to know what you're doing and why to be able to make such decisions.

While getting rid of duplicates results in not enough proofs, it doesn't mean any number of duplicates needs to be
supported. This is especially important for GPU implementation, where diverging control flow kills the performance.
After extensive testing I concluded that supporting just a single duplicate is sufficient. This is also fast on CPU
since handling of the second duplicate is only needed when the first duplicate exists. And most of the time there are no
matches at all, so the CPU can have a decent chance at successfully predicting branches, and GPU threads will mostly
progress without divergence.

## Finding proofs

One place where sorted `y` values are really beneficial is finding proofs. This process involves finding `y` matching
the challenge and propagating back through the tables to generate a proof. But if there are only buckets, this doesn't
work. Another option is to do full scan and since there are buckets, only buckets that overlap with matching range need
to be scanned.

This is substantially slower than binary search, especially when proof doesn't exist. But on the flip side the number of
proofs that need to be checked is upper bound by \\(2^{16}\\), significantly smaller in practice, and the case where
proof isn't found is the minority. So in the end for this number of searches, the increase in proof search time is
overwhelmed with decrease in table construction time, and is a net positive change overall.

Scan is also very CPU-friendly due to predictable behavior (in terms of memory access pattern and branching), so the
performance for when proof is found is actually not far off from the binary search in practice. One of many initially
counter-intuitive cases where doing strictly more work might end up being faster due to how real-world hardware is
designed.

## Memory optimizations

Something I carried in local branches, but didn't really see a massive difference from was pruning the data from parent
tables after they are used. More specifically, both metadata and `y` values are not needed after the next table is
constructed since proof generation is only concerned with `x` values the final `y` was derived from.

Now that there are fewer allocations and things fit better and better into CPU cache, pruning metadata and intermediate
`y` values resulted in performance improvements. Moreover, since tables are no longer sorted by `y`, the position into
the first table is the same as `x` value, so `x` values are not stored anymore and the whole first table is dropped as
soon as the second table is constructed.

All these tricks shaved off about 40% of the memory usage!

I'm very curious to see how this impacts the performance on CPUs with 3D V-cache, where the absolute majority if not all
the data will stay in cache at all times 🔥

## Results

Not every hypothetical improvement results in actual performance improvement. Sometimes it really depends on what fits
into the cache and what doesn't, so it is not obvious when performance will improve and when it will decrease. So all
this took quite a long time with countless benchmark runs and failed experiments.

The changes described above and then some were implemented in [PR 380] and [PR 381].

[PR 380]: https://github.com/nazar-pc/abundance/pull/380

[PR 381]: https://github.com/nazar-pc/abundance/pull/381

The results are still a bit variable due to my machine not staying idle, but when limited to a single CXX on AMD
Threadripper 7970X CPU (roughly equivalent to 8C16T AMD Ryzen 7700X CPU), the results are as follows:

```
Before:
chia/table/single/1x    time:   [920.37 ms 924.24 ms 929.24 ms]
                        thrpt:  [1.0762  elem/s 1.0820  elem/s 1.0865  elem/s]
chia/table/parallel/8x  time:   [677.60 ms 684.25 ms 692.06 ms]
                        thrpt:  [11.560  elem/s 11.692  elem/s 11.806  elem/s]
chia/proof/missing      time:   [20.764 ns 21.179 ns 21.459 ns]
                        thrpt:  [46.600 Melem/s 47.217 Melem/s 48.160 Melem/s]
chia/proof/present      time:   [360.24 ns 360.65 ns 361.08 ns]
                        thrpt:  [2.7695 Melem/s 2.7727 Melem/s 2.7760 Melem/s]
After:
chia/table/single/1x    time:   [747.82 ms 756.94 ms 768.09 ms]
                        thrpt:  [1.3019  elem/s 1.3211  elem/s 1.3372  elem/s]
chia/table/parallel/8x  time:   [529.15 ms 534.42 ms 540.15 ms]
                        thrpt:  [14.811  elem/s 14.969  elem/s 15.119  elem/s]
chia/proof/missing      time:   [101.94 ns 102.22 ns 102.58 ns]
                        thrpt:  [9.7486 Melem/s 9.7824 Melem/s 9.8099 Melem/s]
chia/proof/present      time:   [376.64 ns 377.52 ns 379.55 ns]
                        thrpt:  [2.6347 Melem/s 2.6489 Melem/s 2.6551 Melem/s]
```

As mentioned before, proof searching performance decreased, especially for misses, but it is on average ~4 ms per table,
which is more than compensated by the table construction time improvement.

I still remember the time when we were struggling with table construction for proving. It was taking so long using
reference Chia implementation that we had to introduce the delay into the protocol design to make sure farmers have a
few seconds to generate a proof in time 🥹. Now we casually create tables in 750 ms on a single CPU core and A LOT less
than that when multithreaded (which is the default for proving BTW).

## Optimizations that didn't work out

There were many things that didn't work out, but most notably a version with binary search over sorted `y`s I mentioned
last time. Not needing to sort at all was such a massive win and simplification for both CPU and GPU that I don't think
I will go back to it, but it took a few days to get working reasonably well. Still, it was an interesting experience I'm
sure I'll find applications for in the future.

## Upcoming plans

After implementation was pretty well tuned in Subspace I'm still finding ways to do major performance improvements, even
if it means backwards incompatible changes for farmers. For now, though, I think this is mostly it for the CPU side, and
I'll be switching back to GPU where I'll no longer need to implement custom sorting and other complicated things.

Fingers crossed for the next update in this series to talk about how this all works nicely on GPU with [rust-gpu].

[rust-gpu]: https://github.com/Rust-GPU/rust-gpu

If you have any feedback about this or anything else related to the project, I'm not difficult to find on [Zulip].

[Zulip]: https://abundance.zulipchat.com/
