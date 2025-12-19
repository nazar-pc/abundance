---
title: Permissionless assignments of farmers to shards
date: 2025-12-19
draft: false
description: Making progress on shard-aware consensus changes
tags: [ status-update ]
authors: [ nazar-pc ]
---

[Last time] I shared challenges and next steps in making consensus shard-aware. I'm happy to report that an initial
version of the shard assignment algorithm is now implemented and working.

[Last time]: ../2025-12-11-steps-towards-multi-shard-farming

To the best of my knowledge, this is the first shard assignment design of this kind in blockchain space. All other
designs are permissioned and require on-chain registration with some tokens, which hinders both participation and
scalability.

<!--more-->

The majority of changes are in [PR 476], you can read the description there and higher-level conceptual description in
the previous blog post.

[PR 476]: https://github.com/nazar-pc/abundance/pull/476

The major outcome is that the farmer is mostly prepared to support multiple shards now. I will have to add notifications
about what permutations of public key hash/shard commitments roots/history sizes that connected farmers are currently
using so that node can sync onto corresponding shards. However, since there are no actual shards yet and sync
implementation is still far away, the implementation of it is missing right now, though the RPC interface is already in
place.

From here the next major thing will be to add super segments and figure out all the details about creating the global
history. I explained a high-level idea last time, and I don't really have anything to add to that just yet.

There was a bunch of various fixes throughout last week, but nothing interesting enough to mention here.

## Upcoming plans

The fact that shard allocation is based on slot interval and not beacon chain block interval annoys me quite a bit, so
I'll probably change it soon. It is especially annoying since non-beacon chain blocks do not carry PoT checkpoints in
them, which will make verification even trickier than it already is. Tricky consensus logic is something I'd prefer to
avoid, though.

And I'm kind of itching to experiment with RISC-V interpreter/VM to try to confirm some of my hypotheses.

This update is very short because I had something important to share, but kind of explained it already in the previous
blog post. Ping me on [Zulip] if you have any questions.

[Zulip]: https://abundance.zulipchat.com/
