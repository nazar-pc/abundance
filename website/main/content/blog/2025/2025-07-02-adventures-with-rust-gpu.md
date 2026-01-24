---
title: Adventures with rust-gpu
date: 2025-07-02
draft: false
description: An incomplete attempt to reimplement GPU plotting using rust-gpu
tags: [ status-update ]
authors: [ nazar-pc ]
---

GPU plotting was one of the items on the roadmap last week and that turned into a week+ long side quest, so let me share
some details about that.

<!--more-->

## Background

For a little bit of background, [Subspace protocol] has a compute-intensive plotting component, where the majority of
the cost is Chia-based [Proof-of-Space] that is used to encode plots (it is used differently than in Chia, but that is
not very important here).

[Subspace protocol]: https://subspace.github.io/protocol-specs/docs/protocol_specifications

[Proof-of-Space]: https://subspace.github.io/protocol-specs/docs/consensus/proof_of_space

This is something that can be accelerated with GPU and, in fact, is extremely desirable to make plotting more energy
efficient and less time-consuming. In Subspace GPU plotting was implemented for CUDA/ROCm by very skilled folks from
[Supranational]. Unfortunately, it broke after switching from KZG and some other changes, which left it in unusable
state.

[Supranational]: https://www.supranational.net/

Now I'm trying to avoid C++ when I can, let alone CUDA C++, so I was looking for ways to rewrite it in Rust instead if
possible, which I shared in the past updates. On top of that, previous implementation was picky on AMD side, it only
really worked with RX 6000/7000 GPUs (on the consumer side) and only on Linux. [According to AMD developers] it is
unlikely that Windows support will come any time soon, there were also annoyances caused by
[dynamic linking required by ROCm] to a specific release of their libraries.

[According to AMD developers]: https://github.com/ROCm/HIP/issues/3640

[dynamic linking required by ROCm]: https://github.com/ROCm/ROCR-Runtime/issues/240

And forget about Intel or Apple, iGPUs, etc.

So as you can imagine, I wasn't particularly thrilled with the status quo to begin with. The better situation would be
to target Vulkan/Metal, which is exactly what [wgpu] allows, but we need shaders and I want them in Rust, not yet
another obscure language to suffer with.

[wgpu]: https://github.com/gfx-rs/wgpu

## CubeCL

I looked at CubeCL a few times, tried to write some kernels, but in the end I gave up on it, for now at least. The basic
issue I have with it is that it is one of those obscure shader languages. Yes, it looks like Rust, but it only supports
a subset of language features and standard library, doesn't allow to freely pull `no_std` crates from crates.io and as
the result ends up not being a real Rust.

I strongly considered this path, but ultimately I do want to be able to use Rust with any of its features and have the
fullest control over the compilation output of the code.

## rust-gpu

This brings be back to [rust-gpu], which I initially dismissed due to it requiring old nightly compiler, but
with [cargo-gpu] as a library it became not great, but at least usable. It is in fact actual Rust, more specifically a
codegen backend that rustc calls to produce regular SPIR-V binary, which can be executed on a GPU using wgpu or other
libraries. Using wgpu for this purpose is nice because it'll recompile SPIR-V, which is Vulkan-specific, into shader
that runs on Apple's Metal API, which means support for quite powerful Apple Silicon iGPUs.

[rust-gpu]: https://github.com/Rust-GPU/rust-gpu

[cargo-gpu]: https://github.com/Rust-GPU/cargo-gpu

With that, I started prototyping and learned a lot in progress about rust-gpu, Vulkan, SPIR-V and GPU programming in
general. One interesting property that surfaced fairly quickly was that GPUs really like 32-bit integers and seriously
dislike smaller and larger ones, something like `u128` is not generally available at all, even as an optional
capability.

Not going to lie, it was a struggle. There are countless limitations in rust-gpu as it stands today, and eventually I
hit a wall that I'll probably stop at for now. But I have made progress. When you look at [Proof-of-Space spec], I have
ChaCha8 keystream derivation and `compute_f1()` fully implemented as shaders, and even tested with [LLVMpipe] in CI.
`compute_fn()` is technically implemented, but unfortunately doesn't quite compile despite my best efforts to work
around every last limitation. Since `u64` is not supported on all GPUs and `u128` is not supported anywhere (at least
until rust-gpu learns [to add polyfills] for them), I had to write polyfills for both and test them against native
types, ensuring they have the same exact binary representation in memory.

[Proof-of-Space spec]: https://subspace.github.io/protocol-specs/docs/consensus/proof_of_space

[LLVMpipe]: https://docs.mesa3d.org/drivers/llvmpipe.html

[to add polyfills]: https://github.com/Rust-GPU/rust-gpu/issues/307

That still leaves matching logic and sorting. For sorting, there are some libraries on crates.io, hopefully something
that will work and for matching it shouldn't be terribly difficult to implement directly. CUDA C++ implementation also
did erasure coding, but I didn't look into how difficult it'll be to make [reed-solomon-simd] work with rust-gpu yet.
Worst case we'll do it on CPU for now, erasure coding is A LOT faster now than it was with KZG stuff involved.

[reed-solomon-simd]: https://github.com/AndersTrier/reed-solomon-simd

If you're interested in what I have so far, [PR 313] was the very first piece of code with GPU with a few follow-ups
in [PR 314], [PR 315] and [PR 320].

[PR 313]: https://github.com/nazar-pc/abundance/pull/313

[PR 314]: https://github.com/nazar-pc/abundance/pull/314

[PR 315]: https://github.com/nazar-pc/abundance/pull/315

[PR 320]: https://github.com/nazar-pc/abundance/pull/320

## Other work

Learning more about GPUs makes one re-think some of the existing approaches. As I mentioned earlier, GPUs really do not
like working with individual bytes, but strongly prefer `u32`, so as I was writing GPU code I though if CPU can benefit
from similar changes, which led to [PR 312] with substantial PoSpace verification performance improvement:

[PR 312]: https://github.com/nazar-pc/abundance/pull/312

```
Before:
chia/verification       time:   [9.3419 µs 9.4671 µs 9.7223 µs]
After:
chia/verification       time:   [7.4406 µs 7.4539 µs 7.4651 µs]
```

I was also increasingly frustrated with how difficult it is to land any changes to upstream [BLAKE3] crate and since I
needed BLAKE3 for GPU plotting as well, I ended up creating `ab-blake3` crate. It currently has more exotic and
special-purpose APIs. For example there are `const fn` methods that were under review upstream [since January], there
are also more compact (and thus easier for compiler to optimize) methods that handle up to one chunk and up to one block
worth of data only. For single block version, I also created a portable variant that works with `u32` words instead of
individual bytes, which as you may have guessed is necessary for GPU. In the future there I'd like to have single-block
variants that can process [multiple independent blocks with SIMD].

[BLAKE3]: https://github.com/BLAKE3-team/BLAKE3

[since January]: https://github.com/BLAKE3-team/BLAKE3/pull/439

[multiple independent blocks with SIMD]: https://github.com/BLAKE3-team/BLAKE3/issues/478

Initial implementation landed in [PR 316] with further extension in [PR 318]. Using it in the repo immediately yielded
further small performance improvements:

[PR 316]: https://github.com/nazar-pc/abundance/pull/316

[PR 318]: https://github.com/nazar-pc/abundance/pull/318

```
Merkle Tree before:
65536/balanced/new      time:   [4.1899 ms 4.1903 ms 4.1919 ms]
65536/balanced/compute-root-only
                        time:   [4.2740 ms 4.2743 ms 4.2754 ms]
65536/balanced/all-proofs
                        time:   [1.4789 ns 1.4796 ns 1.4824 ns]
65536/balanced/verify   time:   [70.688 ms 70.696 ms 70.728 ms]

Merkle Tree after:
65536/balanced/new      time:   [3.8788 ms 3.8789 ms 3.8790 ms]
65536/balanced/compute-root-only
                        time:   [3.6719 ms 3.6851 ms 3.6884 ms]
65536/balanced/all-proofs
                        time:   [1.4212 ns 1.4222 ns 1.4225 ns]
65536/balanced/verify   time:   [65.727 ms 65.792 ms 66.050 ms]

PoSpace before:
chia/table/single       time:   [1.0683 s 1.0908 s 1.1156 s]
chia/table/parallel/1x  time:   [158.26 ms 160.78 ms 163.98 ms]
chia/table/parallel/8x  time:   [860.76 ms 873.21 ms 885.64 ms]
chia/verification       time:   [7.4406 µs 7.4539 µs 7.4651 µs]

PoSpace after:
chia/table/single       time:   [977.21 ms 985.74 ms 996.67 ms]
chia/table/parallel/1x  time:   [160.87 ms 162.47 ms 163.84 ms]
chia/table/parallel/8x  time:   [821.04 ms 833.45 ms 850.07 ms]
chia/verification       time:   [6.8722 µs 6.9056 µs 6.9300 µs]
```

## Upcoming plans

With that, I'll probably take a pause with GPU programming and wait for one of many issues/discussions I
opened/commented on in rust-gpu repository to be resolved, so that there isn't as much friction. Still, I think rust-gpu
has a future, I learned a lot about GPUs during the last week and looking forward to returning to this some time soon.
In fact, now that I can write shaders for GPUs, I think there are more protocol components that could be accelerated,
for example, plot auditing might be a good candidate for this.

Now I'll probably be going back to thinking and hopefully prototyping the state management nad Sparse Merkle Tree. I did
some research earlier, but didn't write anything specific in code yet.

Alfonso made some good progress on sharded consensus, so I might go back to that and make plotting/auditing/verification
shard-aware, though it'd be nice to have a farmer and node in runnable shape first.

Basically as much work ahead as ever, but I'm not hard to find on [Zulip] in case you have any thoughts about this
update or the whole project in general.

[Zulip]: https://abundance.zulipchat.com/
