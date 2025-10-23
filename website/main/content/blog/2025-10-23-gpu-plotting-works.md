---
title: GPU plotting works!
date: 2025-10-23
draft: false
description: New Vulkan-based GPU plotting implementation is integrated into the farmer
tags: [ status-update ]
authors: [ nazar-pc ]
---

{{< katex >}}

All the updates in recent weeks were about Proof-of-Space performance improvements, but what was really driving it is my
exploration into how to efficiently implement it for GPU. Today I'm happy to announce that an initial version of that
implementation is integrated into the farmer.

I've tested it on both AMD and Nvidia GPUs, but in principle it should work on any Vulkan 1.2-capable GPU, which
includes both discrete and integrated graphics from something like the last decade or so. It will also run on Apple
Silicon Macs (actually tested in CI) and likely older Macs with Intel/AMD GPUs as well, though I didn't bother verifying
it myself.

The fact that it runs doesn't necessarily mean it is fast, though, so the bulk of this post will be about that.

<p align="center">
<img alt="Screenshot of nvtop CLI with a farmer process in it" src="nvtop-gpu-plotting.png">
</p>

<!--more-->

## Current performance level

I did some quick testing, and performance is not great right now, CUDA/ROCm version that Supranational engineers wrote
for Subspace is currently much faster. I did not do any performance benchmarking or investigation yet, and honestly, it
is not a very high priority, but I'll likely experiment with it some soon. Overall, I'd say it is probably ~5x slower
than the Subspace version today.

I know of a bunch of reasons for it to be the case already, and there are likely many more that I do not know about yet.

One thing that the Subspace version does is erasure coding on the GPU. While erasure coding is way faster without KZG,
it still takes time and is currently implemented sequentially in between creation of proofs on the GPU. This both means
that GPU is idling in the meantime and CPU cores are not utilized properly. Erasure coding could have happened on the
GPU, though, but someone (likely me) needs to write the code for that.

Another thing is that the proofs search is currently implemented in a way that processed all s-buckets regardless of
whether there are proofs at a particular s-bucket or not, although only half of them are needed. Not only that, the
proof as such is actually not needed for plotting either, only its hash is. But the hashes are generated on the GPU too,
pausing GPU work and not even leveraging SIMD fully. In fact, the whole encoding could have happened on the GPU
completely, transferring encoded records to the CPU instead of proofs themselves.

Also, the whole design is such that it tries to leverage the width of the GPU, but the workload is actually quite small,
which probably doesn't fully benefit from the available memory bandwidth. There are a few ways to address this.

What users did with Subspace is simply running multiple instances of the plotter to provide GPU with more opportunities
to hide memory latency. This was necessary because the API was written in a way that made it impossible to do within a
single process. But with the new design it is possible, just not taken advantage of yet.

Another alternative is to fuse the whole plotting pipeline and let each group of 1024 threads (on modern GPUs) process a
single record, with multiple records being processed concurrently. This will use substantially more memory but will
likely be much faster as well since there will be a single fused shader with no global synchronization needed between
processing stages.

There might be also opportunities to optimize the code by removing unnecessary bounds checks, which is one of many
limitations with rust-gpu that prevents me from writing idiomatic Rust code.

So in general there are many already known ways to improve performance before even during any profiling, which I didn't
do either yet.

## The key milestone

But the key milestone here is that the code is written, it does run on the GPU, and it does run correctly. I believe it
is a correct design for performance from an architecture point of view, but more engineering is needed to get it to
actually run fast. [PR 425] is where integration into the farmer happened, I will not mention countless PRs that
implemented various individual shaders before that.

[PR 425]: https://github.com/nazar-pc/abundance/pull/425

## Upcoming plans

This work should unlock moving the farmer crate into the main codebase under `crates`.

I'll probably spend some more time profiling low-hanging fruits since GPU programming is new for me, but the next major
step is to get a basic beacon chain block production locally as mentioned in the previous update.

[Zulip] has been quiet for a while, but I'm there if you have anything to discuss 😉

[Zulip]: https://abundance.zulipchat.com/
