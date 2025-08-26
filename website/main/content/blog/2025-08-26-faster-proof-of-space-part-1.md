---
title: Faster Proof-of-Space (part 1)
date: 2025-08-26
draft: false
description: First part in a series of Proof-of-Space optimizations
tags: [ status-update ]
authors: [ nazar-pc ]
---

{{< katex >}}

In the [last update] I shared that I plan to work on GPU plotting some more, so that is what I did. The "easier" parts
of it were [done] [earlier]. Now it was time for matching logic and that is more complex, so I decided to dedicate the
whole blog post to it.

[last update]: ../2025-08-17-node-prototype

[done]: ../2025-07-02-adventures-with-rust-gpu

[earlier]: ../2025-08-01-client-database-prototype/#gpu-plotting-implementation

<!--more-->

## How is Chia PoSpace used in Subspace?

Chia Proof-of-Space is used in Subspace for plotting, but not in the same way as in Chia itself. Since Subspace is a
Proof-of-Archival consensus, farmers fundamentally store the history of the blockchain itself, but how do we ensure each
farmer stores a unique replica(s) of it? That is exactly where Chia PoSpace comes into play.

We generate Chia tables and use its proofs to encode pieces of the blockchain history. Chia has 4 phases in its
construction, where the first is to create tables and then three more phases compact the tables to get rid of redundant
data. In Subspace only the first phase is needed, which is interesting and has a slightly different set of tradeoffs.

## Chia tables construction speedrun

As a quick summary for those who are not familiar with Chia, the first phase involves the creation of 7 tables. The
first table is more or less takes a seed as an input and generates `y` values for each of \\(2^k\\) `x` value using
`compute_f1()` function (mostly a ChaCha8 stream cipher). Each `y` value has `k+PARAM_EXT` (`PARAM_EXT = 6`) bits. That
is the first table, a set of `x` and `y` values.

The other six tables essentially follow the following process:

* group `y` values into buckets
    * each bucket spans a range of `PARAM_BC = PARAM_B * PARAM_C` (`PARAM_B = 119`, `PARAM_C = 127`, `PARAM_BC = 15113`)
    * this means the first bucket is `y` in the range `0..PARAM_BC`, the second bucket is `PARAM_BC..2*PARAM_BC`, etc.
* take a pair of adjacent buckets (called left and right) and match them
    * for each left `y` derive the target using a special formula
    * check if there is a matching `y` in the right table
* for each match found, compute new `y` and some additional metadata (`x` in the first table's metadata) using a special
  `compute_fn()` function (includes some bit manipulations and [BLAKE3] hashing)

[BLAKE3]: https://github.com/BLAKE3-team/BLAKE3

## Challenges and optimizations

To make the above process faster and more efficient, there are various tricks one can use, which are different for CPUs
and GPUs.

The targets into the right table are typically precomputed on the CPU, which allows avoiding recomputation of a bunch of
multiplications and divisions all the time. To find matches and proofs faster, tables are typically sorted by `y`, so it
is easier to find buckets and `y` in the ranges that relevant.

There are some tricky optimization tricks that are not always intuitive and need to be benchmarked. For example, for
`k=20` the size of the bucket is ~236 elements on average, despite `PARAM_BC` potential "slots" in it (also `y` values
can be present more than once in a bucket). So it turns out, on CPU it is cheaper to copy `y` values into a `PARAM_BC`
-sized array, so that matches can be found in `O(1)` time on CPU, but that is not necessarily faster on the GPU.

The way you sort things is also important and might be different on CPU and GPU due to inherent architecture differences
and differences in optimal memory access and compute patterns.

## Why now?

You might be wondering why I look into optimizations now? Well, turns out implementing the current design on GPU is
quite difficult, and especially difficult (maybe impossible) to do so efficiently. So I was looking for potential
changes that might help with that.

Any proof is valid and can be verified from a consensus point of view. However, from an implementation perspective, we
would really like to make sure both CPU and GPU plotter derive tables and proofs that are the same, so both are
interchangeable. This allows creating plots and prove on CPU-only machine later. This is why efficient implementation on
GPU may require changes to the CPU implementation as well.

## Optimization opportunities

I've been staring at the code and thinking about it for a better part of the last week. I implemented a version that is
closer to the GPU pattern for the CPU, but it is almost twice as slow. It did tell me that it is possible (and will be
relatively easy to port to rust-gpu). But it also made me look closer into the data structures and think about what are
we actually doing in a lot of details.

For example, in [PR 372] I separated the collection of matches and calculation of `y` values and metadata into separate
phases, which I then leveraged in [PR 373] to use SIMD for `compute_fn()`, though due to [this upstream issue] there is
no SIMD acceleration for BLAKE3 hashing there yet.

[PR 372]: https://github.com/nazar-pc/abundance/pull/372

[PR 373]: https://github.com/nazar-pc/abundance/pull/373

[this upstream issue]: https://github.com/BLAKE3-team/BLAKE3/issues/478#issuecomment-3200106103

The offsets for all supported `k` values (CPU implementation supports values between 15 and 25, GPU currently between 15
and 24) fit into `u32`, so that is what precomputed targets into the right table were using. However, by knowing the
left table, we also know the range of `y` values for the right table too. I already mentioned that there are ~236 values
in each bucket on average, and even the most conservative upper bound estimate is below 512. This means that there is no
need to use `u32` for the targets, we can get away with `u16` and cut the size of the data structure 2x, which I did
in [PR 377].

[PR 377]: https://github.com/nazar-pc/abundance/pull/377

## What else can be done?

One more bit of insight into how Subspace uses Chia PoSpace that leads to more optimizations is the fact that we only
use a subset of proofs. In fact, [~2/3 of proofs exist] for `k=20` and since we don't want to waste farmer space (have a
portion of it that isn't used for farming) we actually erasure code the records of the archival history before encoding
with proofs from Chia PoSpace. What this means is that we have \\(2/3*2 = 4/3\\) of proofs necessary in total. So the
encoded plot contains 2/3 of encoded source chunks and 1/3 of encoded parity chunks on average.

[~2/3 of proofs exist]: https://github.com/subspace/protocol-specs/issues/64

What does this mean? Well, since any proof is verifiable, we don't care which ones we use, as long as we're consistent
between CPU and GPU implementations. Or in other words, we can try to drop some information during table construction if
it helps to save RAM/compute, while still having enough proofs to fully encode the plot. Technically, the farmer app is
ready for not enough proofs to be found, but I'm thinking about removing that logic since in practice it never actually
happens and would be detrimental to the farmer rewards.

What can we optimize, you might ask?

We can probably just get rid of `y` duplicates, so when doing matches, we only need to handle present/missing options. I
[did some statistical analysis] and found out that the majority of targets do not have matches, about 1.5% has one match
and going to two, three and more matches decreases by two orders of magnitude with each step. So we can ignore anything
beyond one match and probably not lose too much (though still need to verify the resulting number of proofs
empirically).

[did some statistical analysis]: https://forum.autonomys.xyz/t/potential-table-creation-rules-change-for-the-farmer/4958?u=nazar-pc

Also note that on average there are ~236 `y` values in a bucket, we can truncate the actual number to 256 and then
address it using just `u8` in various places. This also happens to match the minimum work group size in Vulkan, which
can lead to more efficient sorting implementation, etc. Similarly, the number of matches follows the same pattern. So it
is possible to constrain the number of matches to 256 and preallocate the exact number of "buckets" for matches upfront,
rather than dealing with dynamic allocation the way it is done right now.

With all buckets being the same (and small) size, which match Vulkan work group size, I believe it should be possible to
have a massively better performance on GPU than the current Subspace implementation. Moreover, the current
implementation requires 64 kiB of shared memory, which only the most recent GPUs have (Volta/Turning on the Nvidia side
and similarly for AMD). Smaller data types should unlock support for smaller and iGPUs with 32 kiB of shared memory,
making fast GPU plotting more accessible.

In fact, I already started preparing for changes to bucketing, and after introducing new APIs make proofs search
substantially faster in [PR 378] already:

[PR 378]: https://github.com/nazar-pc/abundance/pull/378

```
Before:
chia/proof/missing      time:   [22.638 ns 22.717 ns 22.808 ns]
                        thrpt:  [43.844 Melem/s 44.021 Melem/s 44.174 Melem/s]
chia/proof/present      time:   [362.23 ns 363.37 ns 365.33 ns]
                        thrpt:  [2.7373 Melem/s 2.7520 Melem/s 2.7606 Melem/s]
After:
chia/proof/missing      time:   [19.397 ns 19.608 ns 19.845 ns]
                        thrpt:  [50.390 Melem/s 50.999 Melem/s 51.554 Melem/s]
chia/proof/present      time:   [357.30 ns 358.78 ns 359.90 ns]
                        thrpt:  [2.7785 Melem/s 2.7872 Melem/s 2.7988 Melem/s]
```

## Conclusion

Not all attempts at optimization were successful, there are a lot of changes that never landed and some that are still
floating in my local branches and may only end up in a GPU version in the end. I have done some optimizations already,
here are the results before last week and now:

```
Before:
chia/table/parallel/8x  time:   [767.76 ms 778.72 ms 790.06 ms]
After:
chia/table/parallel/8x  time:   [677.60 ms 684.25 ms 692.06 ms]
                        thrpt:  [11.560  elem/s 11.692  elem/s 11.806  elem/s]
```

I'll continue experimenting with it until the workflow is simple enough to implement efficiently on GPU. Supranational
engineers who implemented the current GPU plotting implementation using CUDA/ROCm did a pretty good job IMO, but knowing
how protocol works more intimately allows for an even more efficient implementation. And we do want the most efficient
implementation possible because that impacts the security of the protocol. If Subspace's `k=20` can be increased to
`k=21` (each increase roughly doubles size/compute) while taking as much time as the previous `k=20` implementation,
that is a win!

While `k` value change will be painful to implement in Subspace, it is possible to implement the change to the way the
farmer stores plots to take advantage of performance improvements. In fact, I have already done it in the past.
Originally, the algorithm for searching of proofs was implementation-defined, and Supranational engineers requested to
change it, or else it would substantially hurt GPU plotting performance. So we introduced `V1` plot version in addition
to the original `V0` to deal with this and supported both for the duration of the testnet. Same or similar thing (like
incremental plot upgrade) could be done again for Autonomys mainnet.

## Bonus content

Something that I implemented using and spent a lot of time trying to optimize during last week was a version of the
matching logic that doesn't create a `PARAM_BC` array. It instead does a branchless SIMD-accelerated binary search over
the whole bucket (which as we remember is upper bound below 512 and practically is often way smaller than that). It uses
the clever approach described in the article [SIMD / GPU Friendly Branchless Binary Search]. It is almost 2x slower than
`O(1)` lookup on CPU, but will most likely shine brightly on GPU (though remains to be benchmarked), especially once the
bucket size is reduced to 256.

[SIMD / GPU Friendly Branchless Binary Search]: https://blog.demofox.org/2017/06/20/simd-gpu-friendly-branchless-binary-search/

I really liked the approach! It is intellectually satisfying to see such tricks working well in practice, despite
technically having worse theoretical algorithm complexity. Knowing how hardware works internally really matters for
performance!

## Upcoming plans

I'll continue experimenting with optimizations and hope to get to GPU side relatively soon. And I will share whatever
results I achieve (or not) in the follow-up update.

In the meantime you can find me on [Zulip], I'd be happy to read any other optimization ideas you might have. I know
there was a lot of brain power poured into Chia optimizations and in fact there are third-party fully compatible
(reportedly, I have not tried them myself) plotters that are substantially faster than the reference implementation
already. Would be cool to learn from that experience.

[Zulip]: https://abundance.zulipchat.com/
