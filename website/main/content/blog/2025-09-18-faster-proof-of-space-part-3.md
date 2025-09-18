---
title: Faster Proof-of-Space (part 3)
date: 2025-09-18
draft: false
description: Third part in a series of Proof-of-Space optimizations
tags: [ status-update ]
authors: [ nazar-pc ]
---

{{< katex >}}

This third part has fewer improvements and could have been called "[Adventures with rust-gpu] part 2" given how much
time I spent wrestling with it.

[Adventures with rust-gpu]: ../2025-07-02-adventures-with-rust-gpu

<!--more-->

## Rmap optimizations

Since the last update I was mostly focusing on GPU implementation for matching logic. As it often happens, I'm
re-reading and re-auditing related CPU code in the process, trying to figure out what would be an efficient way to
implement it. And I discovered one more optimization that turned out to be applicable to both CPU and GPU.

I really want to support as many usable GPUs as possible, which in turn means thinking about their constraints. One of
the constraints is the amount of shared memory used. Turned out that `Rmap` that was essentially `[[u32; 2]; 15113]`
data structure was quite big, almost 121 kiB big! It doesn't fit into the 32 kiB limit that I was targeting, it doesn't
even fit into many modern consumer GPUs with 48-64 kiB of shared memory! BTW, it turned out the baseline for Vulkan is
actually 16 kiB and that is the amount Raspberry PI 5s iGPU has. Using global memory means a massive performance hit,
and even if it could work, if I want to maximize GPU utilization by running more workgroups, I better use less shared
memory than more.

The observation with `Rmap` data structure is that it is sparse. Not only it has most of the slots empty (remember, in
the previous post I mentioned that the bucket size is now limited to just 272 elements), even those that are occupied
usually have 1 element, not two. So a lot of space is wasted. What can be done about that is to create an indirection:
`Rmap` table will store pointers into a different table, which only needs to be `[[u32; 2] 272]` (again, at least half
of it will be empty, but dealing with that is not worth it). Since the second table is much smaller, pointers don't need
to occupy 8 bytes, in fact, 9 bits is sufficient, and that is what GPU implementation uses. For CPU additional
arithmetic operations were not worth it, so it uses `u16` for pointers instead, while still enjoying massive data
structure size reduction.

As a result, we get the following change in CPU performance, which landed in [PR 385]:

```
Before:
chia/table/parallel/8x  time:   [529.15 ms 534.42 ms 540.15 ms]
                        thrpt:  [14.811  elem/s 14.969  elem/s 15.119  elem/s]
After:
chia/table/parallel/8x  time:   [518.48 ms 521.57 ms 525.13 ms]
                        thrpt:  [15.234  elem/s 15.338  elem/s 15.430  elem/s]
```

[PR 385]: https://github.com/nazar-pc/abundance/pull/385

It is not particularly huge difference, but it is certainly consistently faster and has a much higher chance of
remaining in L1 cache during processing.

On GPU using 9 bits for pointers plus an additional table with actual values occupies ~19 kiB of shared memory vs 121
kiB before. This still doesn't fit into 16 kiB on Raspberry PI 5, so the `Rmap` table there will have to be moved to
global memory and hoping that it is cached, but at least everything else fits nicely with some room to spare.

## Other CPU improvements

I was also exploring some other things and noticed that manually unrolling in finding matches was actually worse in the
current state of the library, so I removed that in [PR 388], which further improved performance substantially:

```
Before:
chia/table/single/1x    time:   [747.82 ms 756.94 ms 768.09 ms]
                        thrpt:  [1.3019  elem/s 1.3211  elem/s 1.3372  elem/s]
chia/table/parallel/8x  time:   [518.48 ms 521.57 ms 525.13 ms]
                        thrpt:  [15.234  elem/s 15.338  elem/s 15.430  elem/s]
After:
chia/table/single/1x    time:   [697.77 ms 707.91 ms 723.34 ms]
                        thrpt:  [1.3825  elem/s 1.4126  elem/s 1.4331  elem/s]
chia/table/parallel/8x  time:   [500.34 ms 506.86 ms 513.82 ms]
                        thrpt:  [15.570  elem/s 15.783  elem/s 15.989  elem/s]
```

[PR 388]: https://github.com/nazar-pc/abundance/pull/388

## Finding matches on GPU

Finding matches on GPU was the first kernel that was not embarrassingly parallel and required some level of
synchronization. The reason for it is that most of the candidate pairs do not have matches and those that do need to be
compressed and be in the same deterministic order as on the CPU. Not only that, I later discovered that the whole code
needs to progress in phases or else the [results differ in unexplainable ways] and depend on GPU vendor/implementation.

[results differ in unexplainable ways]: https://github.com/Rust-GPU/rust-gpu/discussions/396#discussioncomment-14443503

It does still waste a bit more time on divergent control flow than I'd like, but we'll see how it performs once the
whole workflow is complete.

Since Raspberry PI 5 and baseline Vulkan requirements more generally are lower than modern-ish consumer dGPUs, I decided
to evolve existing compilation of two kernels with and without `Int64` (64-bit integer) support. Since [PR 389] the
shader variants are "modern" and "fallback." Modern kernel supports `Int64` and 32 kiB+ of shared memory, while fallback
will theoretically run on anything compliant with Vulkan 1.2+, which is most dGPUs in the last decade or so and even
iGPUs. I checked one of the laptops I have with Intel HD Graphics 620 iGPU and even that one should work.

[PR 389]: https://github.com/nazar-pc/abundance/pull/389

## Making it compile with rust-gpu

While I was writing the code, I used `cargo clippy` and `cargo build` for verification, but since I did not have the
shader definition initially, I didn't yet know if it would compile for rust-gpu. And it did not compile in a big way
with over 60 errors.

The root of all evil, or at least the most of it, was [rust-gpu issue 241]. The thing is that I used arrays for many
data structures since I know the upper bound on everything and want to store things compactly. However, I most often
have dynamic indices into those arrays. The way a Rust standard library implements both checked and unchecked indexing
is using `Deref` of an array to a slice, but rust-gpu doesn't allow casing `*[u32; N]` into `*[u32]`, which effectively
means I can't use most of the methods, I can't even use iterators.

[rust-gpu issue 241]: https://github.com/Rust-GPU/rust-gpu/issues/241

So for iterators I had to do manual indexing. For unchecked access just use `[]` without the ability to explain to the
compiler that it doesn't need to do bounds checks. And for checked access I have to first check the index against length
of the array explicitly and then use `[]`. As you can imagine, this resulted in a lot of ugly boilerplate that is much
harder to read. Not only that, I had to give up some of the new types because I couldn't cast new types to/from their
inner types (for those that were backed by reusable scratch space in the shared memory).

All in all, that wasn't a great experience at all, and I hope it'll improve soon.

## Making it run properly

Once the code was compiled, it didn't run properly (surprise!), and I had to figure out why. This turned out to be
[much more challenging than I expected]. Turns out there is no direct way to know if shared executed successfully or
not. If code hits a panic, it just exits execution early, leaving the memory with whatever garbage it may have had
during allocation or with whatever incomplete results it managed to produce so far.

[much more challenging than I expected]: https://github.com/Rust-GPU/rust-gpu/discussions/396

The only real way to know if it completed successfully is to write some value at the very end explicitly and check that
manually. Not only that, once you know that the code has likely panicked, it is surprisingly challenging to figure out
both where and why. No step by step debugger like on CPU for us, not even `println!()` is available easily unless you
implement it yourself. And when you try to implement it yourself, it is most likely that you'll hit the mentioned
pointer casting issue again. Quite a frustrating experience overall, there must be a better way eventually if we want
people to write shaders in Rust.

The complete version of `find_matches_in_buckets` shader with tests landed in [PR 391].

[PR 391]: https://github.com/nazar-pc/abundance/pull/391

## It does run

In the end I managed to make it work, tested both on AMD GPU and with llvmpipe on CPU (which is the one used in CI).
Both produce results that are identical to CPU.

## Upcoming plans

With matching logic implemented, the only remaining difficult thing is bucketing. I think I'll be changing the logic a
bit again, and I think it'll improve performance on CPU too. Once bucketing is implemented, the remaining thing will be
to do proof searching (mostly embarrassingly parallel task) and applying them to record chunks in a sector. Neither of
these two is remotely as complex as bucketing, though.

It'll be a bonus to implement erasure coding on GPU, which will help with both plotting and archiving (which is already
very fast, especially compared to the original KZG-based version in Subspace). But that is an optional thing for now. If
you're reading this and would like to give it a try, let me know!

I'll write once I have more updates to share. In the meantime you can always find me on [Zulip].

[Zulip]: https://abundance.zulipchat.com/
