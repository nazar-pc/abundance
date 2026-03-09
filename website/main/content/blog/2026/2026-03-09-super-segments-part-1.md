---
title: Super segments (part 1)
date: 2026-03-09
draft: false
description: Global history with super segments is designed and partially implemented
tags: [ status-update ]
authors: [ nazar-pc ]
---

It has been a while since the last update. I really wanted to share a completed implementation of super segments, but it
will take some more time, so let's discuss where things are today and how things will work once completed.

<!--more-->

## Global history

The big picture task is to combine all individual shards into a shared system. It includes global archival history in
addition to the relationship between shard blocks from different levels.

What we want to achieve is for each shard to archive its own history locally just like before. However, eventually,
all of those local segments need to be combined into a single linear global history, from which farmers can pick pieces
for plotting purposes.

We discussed a few ways to do that with Alfonso in the past, and here is the design I settled on for now, conceptually:

* a shard produces local segments and corresponding root and local segment number are propagated up to the intermediate
  shard and beacon chain
* beacon chain sees all the local segments and is able to apply reorgs if necessary
* to be confirmed, there must be no reorgs for a particular local segment of the shard observed on the beacon chain for
  a predetermined period of time
* a Merkle Tree is built out of beacon chain's own segments and confirmed lower shard segments, forming a super segment

As mentioned in [Permissionless assignments of farmers to shards], farmers will be reassigned to different shards
periodically. The idea with segment confirmation delay is to pick it such that there is enough time for a diverse set of
farmers to participate in various shards, giving them a chance to land at least one segment reorg on the beacon chain in
case the beacon chain contains invalid segments (which beacon chain nodes can't verify themselves directly).

Also, the presence of deep reorgs visible on-chain can be used for consensus purposes to give more farmers a chance to
produce a block on the shard, effectively increasing the share of farmers verifying the shard and recovering from
disagreements about the shard's state. This will need to be properly analyzed and modeled, though. For now, I am
thinking of 2 shard reassignment intervals as a reasonable starting point for segment confirmation delay.

[Permissionless assignments of farmers to shards]: ../2025-12-19-permissionless-assignments-of-farmers-to-shards

## Implementation

[PR 541] introduced a notion of local segment index, which is an index belonging to a segment history segment of a
shard. It will map to a global segment index once confirmed on the beacon chain.

[PR 541]: https://github.com/nazar-pc/abundance/pull/541

The first issue in the implementation as of the previous update was that the block structure didn't contain information
about shards or local segment indices, just segment roots. In [PR 549] I finished an extremely tedious process of
updating the block structure to include local segment indices of leaf shards in intermediate shards and both local
segment indices and shard indices on the beacon chain.

[PR 549]: https://github.com/nazar-pc/abundance/pull/549

One interesting thing about that is that the information about the lower-level shard needs to be verifiable against the
corresponding block header. Producing block root naively means simply hashing things that are physically present in the
block in the same way they are present in the block. However, it causes issues when trying to confirm the inclusion of
the segment root on the beacon chain where leaf shard block headers are not available. To solve that issue, the shard
index is actually [mixed with the segment root] when building the root of segment headers. This avoids the inclusion of
redundant information in the leaf shard block (which would use space and require additional consistency checks), while
keeping things verifiable on the beacon chain.

[mixed with the segment root]: https://github.com/nazar-pc/abundance/pull/549/changes#diff-6ab9ec9f79e110d362dba28b1e6dc2e5832ab29768175fe6cb33eee16a5d2af9R102-R126

This is not the first time the hashing doesn't follow the block structure literally for technical reasons, but I thought
it was interesting enough to share here.

As mentioned earlier, there will be a parameter that defines the confirmation time/depth for intermediate and leaf shard
segments (and blocks more generally) on the beacon chain, so I renamed older `confirmation_depth_k` to more fitting
`block_confirmation_depth` in [PR 561], which will be complemented by `shard_confirmation_depth` later.

[PR 561]: https://github.com/nazar-pc/abundance/pull/561

In [PR 559] I finally decided what to do about object mapping API in the block archiving task: to remove it. It is
unnecessary for consensus, and for use cases where object mappings are needed, segments can be re-derived again with
object mappings included. This is only about a block archiving task. The archiver as such retains the ability to process
object mappings just like it did before.

[PR 559]: https://github.com/nazar-pc/abundance/pull/559

## Next steps on super segments

Now I'm working on another extremely tedious and uninteresting task of integrating super segments into the code base. It
is a very invasive process, affecting archiving, block production and verification, piece verification after network
downloads, caching, plotting and auditing on the farmer. Essentially, most of the code dealing with archival history one
way or the other is affected, and there is a limited opportunity to land incremental changes.

I landed some refactoring in [PR 565], which I think makes the internal piece structure much more elegant and convenient
to work with. There will be many more fields in it soon to make pieces verifiable against super segments. Part of the
reason why the record is now after other fields rather than before is that with extra fields the piece will contain
shard index, followed closely by the local segment index. I think it is kind of nice that the first few bytes tell you
which shard the piece was archived on.

[PR 565]: https://github.com/nazar-pc/abundance/pull/565

It'll take some time to refactor all the APIs for super segments to be at least somewhat usable. Additional changes will
likely be needed after that since I am taking some shortcuts.

## Remaining consensus work of global history

So far in this post I talked about segments but avoided how blocks are connected together. That is probably the last
conceptual thing to figure out: which beacon chain blocks are supposed to be referenced by intermediate and leaf shards,
which block headers are eligible for inclusion in the higher-level shards (to balance latency and reorgs).

After that, at least for core consensus, what will remain will be to pick the right parameters and thoroughly analyze
the protocol, but mechanically things should be mostly complete.

## RISC-V updates

I did not focus on RISC-V as much last month, but in between my procrastination I have managed to land some
non-negligible improvements.

In [PR 538] I extracted _Zmmul_ and _Zbkc_ sub-extensions. It is common in RISC-V to have a more limited set of
instructions being its own extension, but also included into a bigger extension.

[PR 538]: https://github.com/nazar-pc/abundance/pull/538

I looked a bit into ed25519 signature verification performance and decided to switch back to `ed25519-dalek` crate in
[PR 539]. Pre-release version of `ed25519-dalek` has support for _Zknh_ (SHA2) extension, which I implemented
in [PR 540]. As a result, the benchmark contract size decreased from _68.5 kB_ to _39.1 kB_ with slightly better
performance.

[PR 539]: https://github.com/nazar-pc/abundance/pull/539

[PR 540]: https://github.com/nazar-pc/abundance/pull/540

I then spent some time working on vector extension support, landing instruction decoding for _Zicsr_ and _Zve64x_
instructions in [PR 555]. However, a lot more work is needed to implement the execution part in the interpreter (the
extension is HUGE), which is both about the actual logic and the right abstractions around additional kinds of
registers (so far only general purpose registers were used).

[PR 555]: https://github.com/nazar-pc/abundance/pull/555

I then found some low-hanging fruit. In [PR 563] I added a hint that unaligned reads in contracts are cheap, which
allowed LLVM to generate a lot less machine code, decreasing benchmark contract size from _39.4 kB_ to _34.1 kB_ with
substantial performance improvements. I then decided to switch to 32 general purpose registers in [PR 564]. It
generates better code and makes no difference implementation-wise for a simple interpreter, while more complex
optimizing VM is not happening short-term. By the time optimizing VM with native registers is happening,
hopefully, [APX] will be more common to take advantage of on x86-64.

[PR 563]: https://github.com/nazar-pc/abundance/pull/563

[PR 564]: https://github.com/nazar-pc/abundance/pull/564

[APX]: https://www.intel.com/content/www/us/en/developer/articles/technical/advanced-performance-extensions-apx.html

Here is a rough comparison of benchmarks between the previous update and these improvements I just mentioned:

```
Before:

blake3_hash_chunk/interpreter/lazy
                        time:   [123.31 µs 125.63 µs 131.19 µs]
                        thrpt:  [7.4437 MiB/s 7.7734 MiB/s 7.9194 MiB/s]
blake3_hash_chunk/interpreter/eager
                        time:   [58.122 µs 60.115 µs 63.763 µs]
                        thrpt:  [15.316 MiB/s 16.245 MiB/s 16.802 MiB/s]

ed25519_verify/interpreter/lazy
                        time:   [4.4544 ms 4.5550 ms 4.7086 ms]
                        thrpt:  [212.38  elem/s 219.54  elem/s 224.50  elem/s]
ed25519_verify/interpreter/eager
                        time:   [2.1032 ms 2.1154 ms 2.1251 ms]
                        thrpt:  [470.56  elem/s 472.71  elem/s 475.48  elem/s]

After:

blake3_hash_chunk/interpreter/lazy
                        time:   [75.374 µs 75.493 µs 75.671 µs]
                        thrpt:  [12.905 MiB/s 12.936 MiB/s 12.956 MiB/s]
blake3_hash_chunk/interpreter/eager
                        time:   [34.802 µs 35.240 µs 35.439 µs]
                        thrpt:  [27.556 MiB/s 27.711 MiB/s 28.060 MiB/s]

ed25519_verify/interpreter/lazy
                        time:   [4.0972 ms 4.1289 ms 4.1500 ms]
                        thrpt:  [240.96  elem/s 242.20  elem/s 244.07  elem/s]
ed25519_verify/interpreter/eager
                        time:   [1.7310 ms 1.7451 ms 1.7811 ms]
                        thrpt:  [561.45  elem/s 573.05  elem/s 577.69  elem/s]
```

## Upcoming plans

That is all I have to share for now. Next time I should have super segments fully integrated into the code base and
will, hopefully, not procrastinate too much before then. There might be some RISC-V improvements happening along the way
to distract myself from more boring tasks.

Overall, the progress is being done every week, although slower than expected at times. Until the next update I'll be
on [Zulip] if you have something to discuss.

[Zulip]: https://abundance.zulipchat.com/
