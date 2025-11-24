---
title: The first block
date: 2025-11-24
draft: false
description: Node and farmer implementation are wired together enough to start producing blocks
tags: [ status-update ]
authors: [ nazar-pc ]
---

{{< katex >}}

It has been a month since the last update, and I finally have more exciting news to share here. I received feedback
previously that grinding on the same topic is not particularly interesting, so I decided to wait for something different
to happen, and it finally did, we've got the first block on the beacon chain!

<!--more-->

## Dependencies

It has been a relatively long process to figure out all the data structures and components before the node can be put
together and a farmer can connect to it. On the node side, both block production and import pipelines need to exist, as
well as RPC server for a farmer to connect to.

Basic block data structures and consensus pieces (although subtly broken) were in place for some time already. The
bigger problem was farmer, which both depended on Subspace networking stack and CUDA/ROCm-based GPU plotting. That
GPU-based plotting is what I spent the bulk of my time on last month.

## GPU plotting performance

While I shared that things worked during the last update, performance was not great. I'm happy to report that with Mesa
RADV I was finally able to capture some profiling traces and figure out some bottlenecks. Now the performance matches
the CUDA/ROCm version in Subspace with lower VRAM and shared memory usage and support for a wide range of GPUs.

The biggest one turned out to be vector register pressure with almost all shaders and looked something like this:
<p align="center">
<img alt="Radeon GPU Profiler screenshot before optimizations" src="rgp-before.png">
</p>

As you can see, the shader which generates the second table used 96 vector registers on AMD RX 7600 XT GPU, and that was
limiting GPU occupancy to 5/16 available wavefronts. While not always a problem, it definitely was in this particular
case.

After tinkering with algorithms and rewriting the shaders I ended up with a profile that looks substantially different:
<p align="center">
<img alt="Radeon GPU Profiler screenshot before optimizations" src="rgp-after.png">
</p>

As you can see, both relative time improved and shaders are no longer limited by vector registers pressure. There are
likely other constraints still present, but I am not advanced enough in GPU programming to deal with that yet, and
performance is satisfactory as is.

Let me briefly share some optimizations I have done to achieve that.

## Rmap

`Rmap`, as a reminder, is the data structure that allows to quickly find matches into the right table. It was a source
of optimizations before, and turned out not the last time. The major problems with it were that it was constructed
sequentially and was large, but frequently used data structure. The sequential nature was caused by the fact that there
are duplicated `Y` values in tables, and exactly first two should be used in the sorted order.

I tried solving the sequential nature of its of it in a few different ways. Each attempt was more successful than the
next, but also substantially different, and the ultimate solution is unlike anything before it. And I'm skipping
complete, functional, but ultimately not fruitful results, which I had a few of.

The first attempt was in [PR 430], which added additional sorting steps before matching tables, such that it is
relatively easy to handle duplicates with subgroup operations. Then a piece of additional information is attached to
each `R` value, such that `Rmap` construction can be reduced to a bunch of concurrent atomic "or" operations. I used `R`
instead of `Y` since we know what bucket the value belongs to and with `R` value being in the range of `0..15113` there
were unused bits to store extra information "for free" (in terms of memory usage).

[PR 430]: https://github.com/nazar-pc/abundance/pull/430

This worked better than the sequential version, but there were two additional sorts involved, which takes time, and the
preparation overall was still sequential, even though threads within subgroup cooperated to share values with each
other.

I then looked into parallel preparation. In [PR 431] I parallelized the preparation step, such that all threads are
doing something useful at every step. I had a few more variations of this approach before and after, but they were not
better than this one.

[PR 431]: https://github.com/nazar-pc/abundance/pull/431

This worked slightly better, but the complexity of the preparation step was quite high. Not to mention dreaded register
pressure. And something I kind of accepted at that point was that `Rmap` was still too large to fit into shared memory
on small iGPUs like one found on Raspberry PI 5, so it had to use global memory there. I don't think it'd be too fast
there. So overall I was still unhappy with the result and was looking for alternatives.

And I did implement a drastically different alternative in [PR 435]. I removed extra sorting steps and stopped storing
positions of `R`/`Y` values in `Rmap`. Instead, for each `R` I only reserve two bits, which indicate whether there was a
value present and if so, whether there was a second duplicate found or not. This is the kind of information that can be
efficiently constructed fully in parallel using atomic operations. This is then used to find matches. And only when
doing `compute_fn` step for the matches that were found, a full scan of the right bucket is performed to find the actual
position of `R` value we're dealing with.

[PR 435]: https://github.com/nazar-pc/abundance/pull/435

This feels quite brilliant to me, and I'm wondering why it took me so long to arrive with this simple and elegant
design. Not only it is much simpler, the fact that match doesn't store position of `R` values allowed me to smash all
the necessary information about the match into a single `u32` value: 9 bits for offset of the left value within a
bucket, 6 bits for `m` value, 14 bits for `R` target into the right table, and 1 bit to indicate whether the first or
second duplicate was used. The offset in the left table was not needed strictly speaking, but having it allowed to
further parallelize search for matches with quick sorting of matches afterward (initially implemented in [PR 433]).

[PR 432]: https://github.com/nazar-pc/abundance/pull/433

With that my fighting with `Rmap` was over (at least for now) and it became so small (2 bits per `R` value instead of
9), it now fits nicely on smallest iGPUs in shared memory too!

## Sorting

The bitonic sorting I originally implemented within subgroup registers seemed really nice and elegant at first, I
thought it'd perform really well, but nope! Turns out it was using way too many registers, especially on GPUs with
smaller subgroup sizes. Modern AMD GPUs have a subgroup size of 64, but Nvidia and many other GPUs have 32, which
doubles the number of registers needed to store the whole bucket.

Equipped with Radeon GPU Profiler I was finally able to see that. In [PR 432] I refactored storting to not only sort
the values within shared memory, but also to make the whole workgroup sort the values, rather than subgroups, which made
the problem "wider," and GPUs liked that. The algorithm became much more compact and easy to reason about too. This is
not the first time clever GPU code ended up being slower.

[PR 432]: https://github.com/nazar-pc/abundance/pull/432

## Reducing memory and register usage

A pattern that I became annoyingly familiar with on GPUs is that the memory is scarce (fast memory, like vector
registers and shared memory) and I ended up smashing multiple values into the same `u32` over and over again. For
example, for example, in [PR 433] I was able to combine position and bucket offset to reduce `Match` data
structure from 12 to 8 bytes, which was further reduced to 4 bytes in [PR 435] by storing 4 separate values in one
`u32`. The ALU cost to extract those values paid off every single time!

Noticing that trend, I looked at some of the larger types and noticed metadata. On CPU `u128` Rust type is used for
metadata, which works nicely, but this type is not supported on GPUs. I had to implement polyfills with `u32` and `u64`
to get something that resembles `u128`, but turned out I didn't need all 128 bits, at least not yet. Moreover, `u64`
while is supported by modern GPUs, it required compiling two versions of each shader and after experiments was also
using more vector registers than polyfills that use `u32`.

With that knowledge, I removed the second version of the shader in [PR 436], implemented generic `U32N` polyfll instead
of `U128` in [PR 438] and finally switched to `U32N<3>` for metadata in [PR 439], which both helped to reduce register
usage and reduced memory usage by metadata by 25%. Hypothetically, it is possible to take advantage of it even more
since some tables do not even need three words to store metadata. However, implementing that in a generic way was too
much for the current state of const generics in Rust and I abandoned the idea after a few failed attempts.

[PR 436]: https://github.com/nazar-pc/abundance/pull/436

[PR 438]: https://github.com/nazar-pc/abundance/pull/438

[PR 439]: https://github.com/nazar-pc/abundance/pull/439

Probably one of the most confusing and counter-intuitive sources of register usage today with rust-gpu is loops. It is
extremely common to use the to process batches of elements, yet it causes so much trouble!

For example, originally the shaders were prepared for odd numbers of workgroups to be dispatched (both larger and
smaller than necessary). However, removing that in [PR 432] from all shaders made a substantial improvement in register
usage, despite it being a single loop.

A more extreme example of the same was in [PR 440], where `find_proofs` shader was efficiently loading data from global
memory using 5 nested loops that each were doing exactly two iterations. Simply having those loops meant that to track
their progress, at least 5 `u32` registers were needed, despite only storing two bits of information in each. So I
smashed them all into a single `u32` with a sprinkle of bit shifts. It is not pretty but allowed to achieve full
occupancy and improve performance in that particular shader. I wish there was a more readable way to do it, though.

[PR 440]: https://github.com/nazar-pc/abundance/pull/440

```diff
--- a/crates/farmer/ab-proof-of-space-gpu/src/shader/find_proofs.rs
+++ b/crates/farmer/ab-proof-of-space-gpu/src/shader/find_proofs.rs
@@ -280,12 +280,14 @@ fn find_proofs_impl<const SUBGROUP_SIZE: u32>(

     let mut group_left_x_index = subgroup_local_invocation_id * 2;

-    // TODO: This uses a lot of registers for all the loops and expressions, optimize it further
+    // `chunk_index` is used to emulate `for _ in 0..2` loops, while using a single variable for
+    // tracking the progress instead of a separate variable for each loop
+    let mut chunk_index = 0u32;
     // Reading positions from table 6
-    for table_6_chunk in 0..2 {
+    loop {
         let table_6_proof_targets = subgroup_shuffle(
             table_6_proof_targets,
-            SUBGROUP_SIZE / 2 * table_6_chunk + subgroup_local_invocation_id / 2,
+            SUBGROUP_SIZE / 2 * (chunk_index & 1) + subgroup_local_invocation_id / 2,
         );

@@ -297,10 +299,11 @@ fn find_proofs_impl<const SUBGROUP_SIZE: u32>(

         // Reading positions from table 5
-        for table_5_chunk in 0..2 {
+        chunk_index <<= 1;
+        loop {
             let table_5_proof_targets = subgroup_shuffle(
                 table_5_proof_targets,
-                SUBGROUP_SIZE / 2 * table_5_chunk + subgroup_local_invocation_id / 2,
+                SUBGROUP_SIZE / 2 * (chunk_index & 1) + subgroup_local_invocation_id / 2,
             );

@@ -312,10 +315,11 @@ fn find_proofs_impl<const SUBGROUP_SIZE: u32>(

             // Reading positions from table 4
-            for table_4_chunk in 0..2 {
+            chunk_index <<= 1;
+            loop {
                 let table_4_proof_targets = subgroup_shuffle(
                     table_4_proof_targets,
-                    SUBGROUP_SIZE / 2 * table_4_chunk + subgroup_local_invocation_id / 2,
+                    SUBGROUP_SIZE / 2 * (chunk_index & 1) + subgroup_local_invocation_id / 2,
                 );

@@ -327,10 +331,11 @@ fn find_proofs_impl<const SUBGROUP_SIZE: u32>(

                 // Reading positions from table 3
-                for table_3_chunk in 0..2 {
+                chunk_index <<= 1;
+                loop {
                     let table_3_proof_targets = subgroup_shuffle(
                         table_3_proof_targets,
-                        SUBGROUP_SIZE / 2 * table_3_chunk + subgroup_local_invocation_id / 2,
+                        SUBGROUP_SIZE / 2 * (chunk_index & 1) + subgroup_local_invocation_id / 2,
                     );

@@ -342,10 +347,12 @@ fn find_proofs_impl<const SUBGROUP_SIZE: u32>(

                     // Reading positions from table 2
-                    for table_2_chunk in 0..2 {
+                    chunk_index <<= 1;
+                    loop {
                         let table_2_proof_targets = subgroup_shuffle(
                             table_2_proof_targets,
-                            SUBGROUP_SIZE / 2 * table_2_chunk + subgroup_local_invocation_id / 2,
+                            SUBGROUP_SIZE / 2 * (chunk_index & 1)
+                                + subgroup_local_invocation_id / 2,
                         );

@@ -439,10 +446,39 @@ fn find_proofs_impl<const SUBGROUP_SIZE: u32>(
                                 );
                             }
                         }
+
+                        if chunk_index & 1 == 1 {
+                            break;
+                        }
+                        chunk_index += 1;
+                    }
+                    chunk_index >>= 1;
+
+                    if chunk_index & 1 == 1 {
+                        break;
                     }
+                    chunk_index += 1;
+                }
+                chunk_index >>= 1;
+
+                if chunk_index & 1 == 1 {
+                    break;
                 }
+                chunk_index += 1;
             }
+            chunk_index >>= 1;
+
+            if chunk_index & 1 == 1 {
+                break;
+            }
+            chunk_index += 1;
+        }
+        chunk_index >>= 1;
+
+        if chunk_index & 1 == 1 {
+            break;
         }
+        chunk_index += 1;
     }
 }
```

## Concurrent GPU plotting

One thing I disliked a lot about Subspace's CUDA/ROCm implementation is that it was using [sppark] that is written with
some bad (IMO) architectural decisions. Not only, it doesn't allow to this day to support CUDA and ROCm in the same
process (either CUDA or ROCm needs to be selected at compile time, but not both), it uses global singletons for "GPUs."
What that means is that it is not possible to instantiate the same GPU multiple times and make it run multiple
independent computations concurrently. However, as we discovered experimentally over the years, that is very beneficial
for performance. So users had to run multiple instances of the farmer even with a single GPU just to utilize it fully.

[sppark]: https://github.com/supranational/sppark

Not being tied to that library and relying on Vulkan instead, make it quite straightforward to implement. In [PR 441] I
implemented support for multiple concurrent record encodings on the same GPU with the default being 4 for dGPU and 2 for
iGPU (seems about right from my experiments). The only complication is that `wgpu`'s APIs currently do not support
multiple dispatch queues per GPU, so I had to instantiate multiple "devices" instead, which is probably slightly less
optimal than it would be otherwise, but it works. Finally, no need to open multiple farmers, everything "just works" out
of the box, like it does with CPU plotting and NUMA support there.

[PR 441]: https://github.com/nazar-pc/abundance/pull/441

## Moving farmer and first block production

Now that GPU plotting is no longer a blocker, I did some preparation by moving crates in [PR 437] and [PR 444] (mostly
networking stack and some primitives). Then in [PR 445] `ab-farmer` was finally created from `subspace-farmer`.

[PR 437]: https://github.com/nazar-pc/abundance/pull/437

[PR 444]: https://github.com/nazar-pc/abundance/pull/444

[PR 445]: https://github.com/nazar-pc/abundance/pull/445

Having a farmer is nice, but the node was not complete, the biggest missing piece on the surface was the lack of RPC
server to connect to. I added one in [PR 448] alongside a bunch of fixes (some of which were extracted into [PR 446])
and previously lacking consensus archiving implementation.

[PR 446]: https://github.com/nazar-pc/abundance/pull/446

[PR 448]: https://github.com/nazar-pc/abundance/pull/448

With that, the first blocks were produced:

```
2025-11-23T19:49:38.853648Z  INFO ab_node::cli::run: ✌️ Abundance 0.0.1
2025-11-23T19:49:38.853674Z  INFO ab_node::cli::run: 📋 Chain specification: dev
2025-11-23T19:49:38.853680Z  INFO ab_node::cli::run: 💾 Database path: /tmp/ab-node-HctAVm
2025-11-23T19:49:38.853961Z  INFO ab_node::cli::run: Started farmer RPC server address=127.0.0.1:9944
2025-11-23T19:49:38.853978Z  INFO ab_client_archiving::archiving: Not creating object mappings
2025-11-23T19:49:38.853989Z  INFO ab_client_archiving::archiving: Starting archiving from genesis
2025-11-23T19:49:38.853995Z  INFO ab_client_archiving::archiving: Archiving already produced blocks 0..=0
2025-11-23T19:50:31.609669Z  INFO ab_client_block_authoring::slot_worker: 🔖 Built new block slot=27 number=1 root=d5217acac57cf25f598630b87ad9b4c4bfdc9b3f41c5d51badf1fed3a052375f pre_seal_hash=ad3d9fbddcbb178ace0f2f3d1a0d1b23dc54deeea726d9cae29e82a20025ee18
2025-11-23T19:50:31.613833Z  INFO ab_client_block_import::beacon_chain: 🏆 Imported block number=1 root=d5217acac57cf25f598630b87ad9b4c4bfdc9b3f41c5d51badf1fed3a052375f
2025-11-23T19:50:46.991342Z  INFO ab_client_block_authoring::slot_worker: 🔖 Built new block slot=36 number=2 root=37e75ce4794c99d6ede6ae1dbc14fb3ce3fbeaff144b3a2971c6cffb80771d74 pre_seal_hash=f492d7264ca3b05aae49f1a88107b48c6bd9c1ff6ee6a352add7bbd9758fa6fc
2025-11-23T19:50:46.994170Z  INFO ab_client_block_import::beacon_chain: 🏆 Imported block number=2 root=37e75ce4794c99d6ede6ae1dbc14fb3ce3fbeaff144b3a2971c6cffb80771d74
2025-11-23T19:50:50.425401Z  INFO ab_client_block_authoring::slot_worker: 🔖 Built new block slot=38 number=3 root=5dbbbf2468e60e52edf57ddbe4ced4cefe35b1754cafdc139abba1edd86de973 pre_seal_hash=099d9d2e79bf854d720181482317bbf44920c43ce9e63303128b4b64a16c2a22
2025-11-23T19:50:50.428264Z  INFO ab_client_block_import::beacon_chain: 🏆 Imported block number=3 root=5dbbbf2468e60e52edf57ddbe4ced4cefe35b1754cafdc139abba1edd86de973
2025-11-23T19:50:52.120060Z  INFO ab_client_block_authoring::slot_worker: 🔖 Built new block slot=39 number=4 root=a3b7e8fa84a33996e80c57bfeff7c9cf4187203f488a6534a611a8ce0ec23d0d pre_seal_hash=f21c74285fa56ffb1c18b5d90e08e87be929bb590107c61cafbce80b5ab32571
2025-11-23T19:50:52.122686Z  INFO ab_client_block_import::beacon_chain: 🏆 Imported block number=4 root=a3b7e8fa84a33996e80c57bfeff7c9cf4187203f488a6534a611a8ce0ec23d0d
```

There is a lot of work ahead now, but having something like this working is an important milestone nevertheless.

This is a blockchain node built from scratch, no Substrate or anything like that.

## Upcoming plans

I am aware of some mismatch between GPU and CPU plotter, so I'll have to look into that soonish (GPU is used
automatically, so I'm now testing it all the time by simply starting a farmer).

I'll probably do some updates to the node and start tackling countless TODOs everywhere before considering adding
support for intermediate and leaf shards into the mix.

The major items will be working on the database again, there are too many TODOs there, and there are some architectural
changes needed to support different kinds of shards.

The networking stack is something I'm not particularly happy with. I'm not even 100% sure about going with libp2p
anymore, its APIs are quite cumbersome to use, and I have a persistent feeling that there must be a better way.

And I've been doing occasional research about RISC-V interpreter/VM design. I have more answers for myself now, but
ideally, I'm looking for someone with experience to work on this for not a lot of money, so if you know someone who
might be interested, please let me know.

With that, the post is long enough as is. I'll plan for more frequent updates again in the future, a month was the
longest gap between posts so far. In case you are curious about something sooner, [Zulip] is a good place to ping me.

[Zulip]: https://abundance.zulipchat.com/
