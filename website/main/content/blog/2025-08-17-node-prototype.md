---
title: Node prototype
date: 2025-08-17
draft: false
description: Introduction of the `ab-node` crate with a bunch of placeholder features
tags: [ status-update ]
authors: [ nazar-pc ]
---

The state of the codebase is slowing approaching the state in which block production might be finally possible.
Exciting!

<!--more-->

## Node prototype

I've spent a lot of time thinking about how to organize the state both in memory and on disk exactly, not just
conceptually. That has proven to be challenging, so I shelved it for now and did the most basic thing possible
in [PR 362]: store state in memory and hash everything on every block.

[PR 362]: https://github.com/nazar-pc/abundance/pull/362

Interestingly, this is not that far off from how the beacon chain and intermediate shards will work in the end, but for
user transactions on leaf shards a completely different approach will be needed. But we can deal with that later and
potentially in parallel with other issues.

With that I went on to create a prototype of the `ab-node` crate in [PR 364] with implementation of the database
formatting command (as I described in the [earlier blog post]). There were several bugs in the database implementation
that even this basic functionality helped to uncover and fix.

[PR 364]: https://github.com/nazar-pc/abundance/pull/364

[earlier blog post]: ../2025-07-20-sparse-merkle-tree-and-client-database-preparation#client-database

With that I prototyped a basic beacon chain block production and import workflow in [PR 367], which is hypothetically
almost sufficient to produce blocks locally without any networking. We'll know if it actually works once there is a
farmer connected to it, which is currently not the case.

[PR 367]: https://github.com/nazar-pc/abundance/pull/367

The things currently missing for that:

* farmer RPC layer (based on JSON-RPC like it was in Substrate, but since this is an RPC dedicated just for the farmer,
  it might be replaced with something binary later)
* `subspace-networking` needs to be ported to `ab-networking`, likely more or less as it is right now
* GPU plotting implementation needs to be in a shape that kind of works, so `subspace-farmer` can migrate to it
* `subspace-farmer` needs to become `ab-farmer`

With those completely feasible steps, it should be possible to produce blocks in a loop with a local farmer. Since the
whole consensus workflow works outside the block runtime, the fact that block execution is not yet implemented should
not cause issues for now.

Once that is done, the farmer will need to be upgraded with a notion of commitment to a leaf shard. Node will need to be
extended to support intermediate and leaf shards, and a lot of parallel streams of work open up at that point.

## Upcoming plans

Those have been the key updates since the last blog post.

I'll focus on GPU some more during the upcoming week and will look into wiring a basic RPC layer for the farmer on the
node side.

I think in about two weeks there should be a way to run both node and farmer together, depending on how many hacks and
placeholders I incorporate 😅

I'll keep this update shorter. In case of questions or just feedback about these updates (what you find
interesting/useful, what is less so), ping me on [Zulip] in one of the relevant channels.

[Zulip]: https://abundance.zulipchat.com/
