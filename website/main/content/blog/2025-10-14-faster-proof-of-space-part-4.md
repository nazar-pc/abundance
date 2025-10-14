---
title: Faster Proof-of-Space (part 4)
date: 2025-10-14
draft: false
description: Fourth part in a series of Proof-of-Space optimizations
tags: [ status-update ]
authors: [ nazar-pc ]
---

{{< katex >}}

It has been a couple of weeks since the last status update about performance improvements in Proof-of-Space and I am
finally at a decent stopping point where all architectural changes are done and I can share them with you.

<!--more-->

## Matching performance optimizations

In a long series of observations, I noticed that when matching proofs, we always need both `y` values and its position,
which were previously stored separately. Combining them into a tuple with some extra cache line alignment for some data
structures in [PR 393] resulted in yet another substantial performance improvement, both for table generation and even
proof searching:

```
Before:
chia/table/single/1x    time:   [702.17 ms 707.33 ms 712.59 ms]
                        thrpt:  [1.4033  elem/s 1.4138  elem/s 1.4242  elem/s]
chia/table/parallel/8x  time:   [502.50 ms 505.77 ms 509.35 ms]
                        thrpt:  [15.706  elem/s 15.818  elem/s 15.920  elem/s]
chia/proof/missing      time:   [107.29 ns 108.80 ns 111.58 ns]
                        thrpt:  [8.9618 Melem/s 9.1915 Melem/s 9.3207 Melem/s]
chia/proof/present      time:   [374.03 ns 375.88 ns 377.16 ns]
                        thrpt:  [2.6514 Melem/s 2.6604 Melem/s 2.6736 Melem/s]
After:
chia/table/single/1x    time:   [638.21 ms 642.97 ms 649.77 ms]
                        thrpt:  [1.5390  elem/s 1.5553  elem/s 1.5669  elem/s]
chia/table/parallel/8x  time:   [426.94 ms 431.49 ms 436.70 ms]
                        thrpt:  [18.319  elem/s 18.540  elem/s 18.738  elem/s]
chia/proof/missing      time:   [76.806 ns 76.913 ns 77.038 ns]
                        thrpt:  [12.981 Melem/s 13.002 Melem/s 13.020 Melem/s]
chia/proof/present      time:   [351.19 ns 352.22 ns 354.05 ns]
                        thrpt:  [2.8245 Melem/s 2.8391 Melem/s 2.8474 Melem/s]
```

[PR 393]: https://github.com/nazar-pc/abundance/pull/393

## Proof searching and verification improvements

In [PR 394] I then further reduced memory usage and simplified public API that allowed to reuse `LeftTargets` across all
table generation instances, which helps with higher-level CPU cache utilization. As a bonus this PR removes heap usage
from proof verification and fixes (previously broken) `alloc` feature for a measurable verification performance
improvement:

```
Before:
chia/verification       time:   [7.0819 µs 7.0908 µs 7.0961 µs]
                        thrpt:  [140.92 Kelem/s 141.03 Kelem/s 141.21 Kelem/s]
After:
chia/verification       time:   [6.4624 µs 6.4689 µs 6.4758 µs]
                        thrpt:  [154.42 Kelem/s 154.59 Kelem/s 154.74 Kelem/s]
```

[PR 394]: https://github.com/nazar-pc/abundance/pull/394

## Proofs API simplification

I mentioned in one of the previous updates that number of matches for a pair of buckets was limited to 288 (a multiple
of 32, important for GPUs), and the number of elements per bucket of `y` values was limited to 272 (a multiple of 16,
important for CPU). The idea behind those numbers is that they were supposed to be the smallest numbers that still
provide enough proofs for sector encoding. I came up with those numbers after a series of empirical experiments, but
didn't have any guarantees that the assumption will hold.

Turns out, it was crucial for further simplification. Farmer since at that time the farmer code was ready for
an insufficient number of proofs for a sector, which complicates the logic in multiple places, especially when GPU
implementation is involved. In [PR 408] I added a test case (it was much harder to write a `const fn` function that
prevents compilation) to ensure that the assumption always holds and removed support for unencoded sector chunks from
the farmer.

[PR 408]: https://github.com/nazar-pc/abundance/pull/408

With that, it was possible to simplify proof handling. [PR 410] introduced and started using a new API that instead of
generating tables that can be queried later, generates a sufficient number of proofs and a bitmap with which proofs are
present. The bitmap is exactly the same as the one farmer was already generating internally, so a bit of compute is
saved there. The proofs themselves are also easier to handle, especially during piece decoding.

[PR 410]: https://github.com/nazar-pc/abundance/pull/410

After looking for so long at the way proofs are found, I also noted that the current design is somewhat inspired by the
older architecture. In the older architecture the tables were sorted by `y` values. In Chia this is important. However,
we're only interested in a single proof per challenge, so [PR 406] changed the way s-buckets are converted into the
challenges that Chia deals with to optimize the search (breaking change compared to Subspace implementation), and
then [PR 411] refactored internals to only retain a single proof target per s-bucket instead of the complete last
seventh table. With the above changes, we can generate the tables and find the proofs after than the earlier split
process of generating full tables and then finding proofs one by one:

```
Before:
chia/proofs/single/1x   time:   [728.10 ms 735.58 ms 744.74 ms]
                        thrpt:  [1.3427  elem/s 1.3595  elem/s 1.3734  elem/s]
chia/proofs/parallel/8x time:   [600.14 ms 604.93 ms 609.18 ms]
                        thrpt:  [13.132  elem/s 13.225  elem/s 13.330  elem/s]
After:
chia/proofs/single/1x   time:   [710.02 ms 713.94 ms 718.19 ms]
                        thrpt:  [1.3924  elem/s 1.4007  elem/s 1.4084  elem/s]
chia/proofs/parallel/8x time:   [567.42 ms 574.66 ms 581.51 ms]
                        thrpt:  [13.757  elem/s 13.921  elem/s 14.099  elem/s]
```

[PR 406]: https://github.com/nazar-pc/abundance/pull/406

[PR 411]: https://github.com/nazar-pc/abundance/pull/411

Note that while the numbers are a bit higher than before, this is not just table generation anymore, this is the time to
find all the proofs the farmer will need for record encoding. And now since the API is reduced in scope, more
optimizations are possible (though not implemented yet). For example, sector encoding is actually done with a hash of
the proof. Since we know we'll have exactly 2^15 proofs, we can use BLAKE3 SIMD to hash them much faster and return
hashed proofs to the caller, which is even less RAM and much faster. Overall, there are several new optimization
opportunities remaining unimplemented. A substantial chunk of the logic is actually sequential there for now, and yet it
is much faster than before I started all these optimizations.

## GPU implementation

A lot of the changes above were driven by observations of what is actually needed for the protocol and what is an
implementation design decision and can be changed. Some things were more efficiently implementable on GPU when design is
changed slightly, and it turns out most of those changes benefit CPU as well. It is really beneficial to know both the
theoretical needs of the protocol and the nuances of what can be efficiently implemented for CPU/GPU at the same time.

Overall, I have managed to implement all the pieces needed for plotting on the GPU.

[PR 395] implemented a shader for sorting individual buckets. CPU produces deterministic order due to single-thread
implementation (parallelization is not worth it there), but it is too costly on the GPU, so everything is placed in
arbitrary order and sorted after the fact. Since the number of matches is small and has a hard upper-bound of less than
512, I ended up using bitonic sort and modified it in [PR 396] to only use registers, which I think is a fairly neat
implementation and should shine on a GPU.

[PR 395]: https://github.com/nazar-pc/abundance/pull/395

[PR 396]: https://github.com/nazar-pc/abundance/pull/396

To optimize the memory bandwidth, I fused matching shader and `compute_fn` shaders together in [PR 401], then similarly
fused `chacha8` and `compute_f1` shaders into one as well in [PR 402].

[PR 401]: https://github.com/nazar-pc/abundance/pull/401

[PR 402]: https://github.com/nazar-pc/abundance/pull/402

I then introduced `find_matches_and_compute_last` variant in [PR 414], which instead of grouping entries by buckets,
stores proof targets like I described above. However, in contrast to the CPU version that is sequential and picks the
first entry per s-bucket, this one had to also estimate an upper-bound for number of elements per s-buckets and reduce
them to a single one at a later stage.

[PR 414]: https://github.com/nazar-pc/abundance/pull/414

And with all of those, [PR 415] finally implemented `find_proofs` shader, which looks at those s-bucket elements,
reduces
each to a single one and finally generates proofs. It doesn't hash the proofs yet, which, as I mentioned above, would be
a nice performance improvement. It also processes even entries that do not have proofs, which wastes some amount of
compute that could have been used better. But those are optimizations for another time.

[PR 415]: https://github.com/nazar-pc/abundance/pull/415

## Upcoming plans

Now all the primitives necessary for GPU plotting are present, what's left is to combine those shaders into a single
pipeline. Should not be too hard and GPU plotting will be ready.

With that I should be able to move the farmer and implement a version of local beacon chain block production shortly
afterward. And after that I'll probably get back to the database, it needs more work with upper-bound estimation to be
able to run multiple shards in the same physical file/disk, which is crucial for the introduction of intermediate and
leaf shards.

Hopefully future updates will be more entertaining, in the meantime you can find me on [Zulip].

[Zulip]: https://abundance.zulipchat.com/
