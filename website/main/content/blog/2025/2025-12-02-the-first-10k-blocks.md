---
title: The first 10k blocks
date: 2025-12-02
draft: false
description: More functional node implementation can produce large number of blocks now
tags: [ status-update ]
authors: [ nazar-pc ]
---

In the previous update I shared that the block production started to work on the beacon chain. Well, it did produce the
first block, but not that many more, for a variety of reasons. But majority of those reasons are not fixed, so I'm happy
to share what they were and where things are at today.

<!--more-->

```
2025-12-02T18:30:30.293062Z  INFO ab_client_block_authoring::slot_worker: 🔖 Built new block slot=80592 number=13621 root=96fd67e51a475dd17e8d52f192ab77716f4cf11fe8f10e013541c9637ffb9498 pre_seal_hash=65d47b1db13230898027763f09f941795c2005c76acaf84eafec8aa3f3275d6b
2025-12-02T18:30:30.301322Z  INFO ab_client_block_import::beacon_chain: 🏆 Imported block number=13621 root=96fd67e51a475dd17e8d52f192ab77716f4cf11fe8f10e013541c9637ffb9498
2025-12-02T18:30:30.795821Z  INFO ab_client_informer: 💤 shard=BeaconChain best_number=13621 best_root=96fd67e51a475dd17e8d52f192ab77716f4cf11fe8f10e013541c9637ffb9498
2025-12-02T18:30:31.293669Z  INFO ab_client_block_authoring::slot_worker: 🔖 Built new block slot=80597 number=13622 root=f7f8f5fd64c433be9954e3e8f7d40c1c54e8517d0fa7a56b14e5fda46f1a56be pre_seal_hash=7fbd8c6eff83cc1fcc4ec368366ad3988fedd2e6b525b939ed49b64cd15e5211
2025-12-02T18:30:31.301710Z  INFO ab_client_block_import::beacon_chain: 🏆 Imported block number=13622 root=f7f8f5fd64c433be9954e3e8f7d40c1c54e8517d0fa7a56b14e5fda46f1a56be
2025-12-02T18:30:31.493792Z  INFO ab_client_block_authoring::slot_worker: 🔖 Built new block slot=80598 number=13623 root=0313cd2d5d70c7ae299bdadaf595e66fbfaa945cf6f8826fe5f6bb233e579289 pre_seal_hash=d8cf050a2229b56a303f0cb4ffa3e6a04b6bef904ac85179d7769f4f32ca9fb3
2025-12-02T18:30:31.501569Z  INFO ab_client_block_import::beacon_chain: 🏆 Imported block number=13623 root=0313cd2d5d70c7ae299bdadaf595e66fbfaa945cf6f8826fe5f6bb233e579289
2025-12-02T18:30:32.694473Z  INFO ab_client_block_authoring::slot_worker: 🔖 Built new block slot=80604 number=13624 root=3d80b9ecf7a5f2897cb08e1c64bc29508fbfed02d5112fc0601a37e38222624f pre_seal_hash=1eac2eb7389a64c607cab88f8ce53cf8ff8edeea9e7921e8fb71d0b805b3f6e1
2025-12-02T18:30:32.702264Z  INFO ab_client_block_import::beacon_chain: 🏆 Imported block number=13624 root=3d80b9ecf7a5f2897cb08e1c64bc29508fbfed02d5112fc0601a37e38222624f
2025-12-02T18:30:34.095277Z  INFO ab_client_block_authoring::slot_worker: 🔖 Built new block slot=80611 number=13625 root=8b87d0e0e41c97ec7818d5d7394061d360c6396915a93949b94c73e0ef547794 pre_seal_hash=427625877e63ed57c09c52a9927e8f8d7be32abc127b655ed1503b4968c56fe3
2025-12-02T18:30:34.103259Z  INFO ab_client_block_import::beacon_chain: 🏆 Imported block number=13625 root=8b87d0e0e41c97ec7818d5d7394061d360c6396915a93949b94c73e0ef547794
2025-12-02T18:30:35.797046Z  INFO ab_client_informer: 💤 shard=BeaconChain best_number=13625 best_root=8b87d0e0e41c97ec7818d5d7394061d360c6396915a93949b94c73e0ef547794
2025-12-02T18:30:37.297141Z  INFO ab_client_block_authoring::slot_worker: 🔖 Built new block slot=80627 number=13626 root=6942ff43d98f35dad0ff1668cb9068b03599aa14f3fbebc2d6a276ce4aa05756 pre_seal_hash=6ec425df17e120f46aa888575cd59af1fc04b9724110122b935f63d6dbc5bb73
2025-12-02T18:30:37.305484Z  INFO ab_client_block_import::beacon_chain: 🏆 Imported block number=13626 root=6942ff43d98f35dad0ff1668cb9068b03599aa14f3fbebc2d6a276ce4aa05756
```

## CPU/GPU plotting mismatch

The first issue was that occasionally I was getting strange issues about either insufficient quality or other consensus
errors, but only when plotting with GPU. It took me a few days to track this down due to this being a very rare issue
that was not reproducible with generated inputs in integration tests. Eventually I fixed it in [PR 452], it turned out
that the CPU version was relying on some assumptions that were no longer true and that the GPU version correctly took
into consideration. With that I did not get those strange errors anymore, but there were plenty of others.

[PR 452]: https://github.com/nazar-pc/abundance/pull/452

## Archiving issues

When 100 blocks were produced, archiver attempted to archive the block by reading it from the database, but the database
had a TODO instead of block reading implementation. I initially thought the block would be in RAM and that it would be
able to proceed anyway. However, I recalled that wrong. The block body was only present on the disk at that point, so I
had to implement block reading.

I changed the block reading API to async and implemented it in [PR 457]. In the process of doing so, I had to fix a few
more issues, notably offsets when it comes to reading from disk were in pages instead of bytes, which made writes
messing the data and reading it back resulted in garbage. There was also a block confirmation issue. And while at it, I
improved internals a bit with some refactoring. It is all still quite unpleasant to work with, but I don't have a good
idea how to improve it drastically yet.

[PR 457]: https://github.com/nazar-pc/abundance/pull/457

## PoT issues

With those out of the way, I was able to produce more blocks until the point where entropy was supposed to be injected
into PoT, at which there were a bunch of issues found. From minor block decoding error to wrong sources of PoT parameter
changes. The big item, though, was TODO in place of the notification from the block import to the PoT source worker,
which caused PoT entropy to not be injected at all, but block verification was still expecting it.

All those and some more were fixed in [PR 458] and [PR 459]. With these fixes the beacon chain is able to produce over
10k blocks with no issues, going through PoT entropy injection, solution range adjustments, etc. Very similarly to how
[Subspace] works, though a lot of things are reimplemented from scratch slightly differently.

[PR 458]: https://github.com/nazar-pc/abundance/pull/458

[PR 459]: https://github.com/nazar-pc/abundance/pull/459

[Subspace]: https://subspace.github.io/protocol-specs/docs/category/consensus

## Other nice to have things

Feature-wise, I have implemented an "informer" in [PR 454], which just like in Substrate prints a message every five
seconds with the current state of the blockchain, you can see it in the logs above.

[PR 454]: https://github.com/nazar-pc/abundance/pull/454

To make debugging more pleasant, I added a tiny benchmark in the timekeeper in [PR 455]. Now, when that benchmark
detects that the slot was produced way too quickly, it will switch to a different mode where it sleeps after creation of
PoT checkpoints to maintain expected slot time. This way everything progresses as expected, while the actual CPU usage
for PoT proving is essentially zero instead of burning a full CPU core at all times.

[PR 455]: https://github.com/nazar-pc/abundance/pull/455

And lastly, in [PR 456] I went through all `unsafe` code, ensured all usages have safety comments, and enabled Clippy
lint to check that in CI.

[PR 456]: https://github.com/nazar-pc/abundance/pull/456

## Upcoming plans

That is it for now, the implementation is much less buggy now than at the time of the previous update, but I'm sure
there are still plenty of things left to fix as well.

I'll be going through various TODOs. The database pruning is still not implemented, and I'll probably postpone it
because I'm not going to run the node for days any time soon anyway. I'll need to implement bounded block size, though,
so the database size can be estimated and pre-allocated, instead of guessing. Once that is done, it'll be possible to
know how many shards a node can run concurrently within allocated disk space (there will be at least the beacon chain,
one intermediate shard and one leaf shard, possibly more). Still not sure what to do about contracts caching, but I
guess I'll figure it out once I get there.

The networking stack is still an open question, but I'll probably not need/be blocked by it for some time still. I'd
really like something relatively simple and photobuf-free, and libp2p is neither at the moment, especially Rust version.

RISC-V VM is also in the background, but I might start some experimentation to get something functional off the ground
myself. That will be a big and probably both fun and frustrating project.

As always, if you have any questions, you can find me on [Zulip].

[Zulip]: https://abundance.zulipchat.com/
