---
title: Beacon chain block processing infrastructure
date: 2025-06-16
draft: false
description: Basic components for creating and importing beacon chain blocks
tags: [ status-update ]
authors: [ nazar-pc ]
---

There was no update last week again since I didn't feel like there was enough to share at the time, but now that more
things have settled I'd like to share what I've been busy with.

<!--more-->

## Block production/import work

As mentioned in last few updates, block production remained the goal. I'm currently focusing on beacon chain block, but
APIs overall should be later applicable to intermediate and leaf shard blocks as well. Each kind of shard has its own
nuances, so I'm focusing on one at a time.

Consensus in Subspace is coupled with Substrate quite tightly, so I focused on reducing that in [PR 273] and [PR 274].
[PR 274] introduced generic block-related traits, extended in [PR 275], such that block production and import APIs can
describe abstract interfaces for any kind of block being produced/imported.

[PR 273]: https://github.com/nazar-pc/abundance/pull/273

[PR 274]: https://github.com/nazar-pc/abundance/pull/274

[PR 275]: https://github.com/nazar-pc/abundance/pull/275

With all that in place, [PR 278] finally introduces slot worker and initial abstract interfaces for block production. It
was just an abstract interface that doesn't care about specific kind of shard. [PR 280] was built on top and implemented
a prototype of beacon chain block production, which was extended to block import in [PR 286], though "import" doesn't
have any block execution yet, just verification of consensus rules.

[PR 278]: https://github.com/nazar-pc/abundance/pull/278

[PR 280]: https://github.com/nazar-pc/abundance/pull/280

[PR 286]: https://github.com/nazar-pc/abundance/pull/286

At this point a lot of things are still missing: block execution doesn't exist, MMR is not missing, child shards are not
a thing yet either, state management, the list goes on... It is not really usable yet, but with a few more steps it
should be able to actually build and import blocks, at least locally. BTW, block import is conceptually split into
verification and execution, which should help with light client implementation.

One key difference from Subspace, which I think will help with reviews and understanding of the code, is that it assumes
availability of sufficient number of recent blocks for consensus purposes in RAM instead of reaching out to state all
the time. This required a special API of the block verification/import, but as the result avoids storage and maintenance
of intermediate values, which results in a bunch of tricky logic that makes thing look more complex than the protocol
specification would suggest. This, again, helps light clients to reuse the code of the reference implementation and
avoid the need to support execution environment in any capacity, at least for now.

## Other changes

[PR 282] simplified implementation and improved Merkle Tree performance a bit, I'm quite pleased with how it looks right
now even though more performance is still left on the table. After that [PR 283] introduced `no-panic` feature to Merkle
Tree implementation, guaranteeing that its implementation is completely panic-free (except memory allocation code, of
course), which together with extensive tests and Miri should provide strong reliability guarantees. I'll soon be working
on Merkle Mountain Range, which will likely share some code with unbalanced Merkle Tree implementation. To make sure
performance doesn't regress, [PR 288] finally implemented Merkle Tree benchmarks.

[PR 282]: https://github.com/nazar-pc/abundance/pull/282

[PR 283]: https://github.com/nazar-pc/abundance/pull/283

[PR 288]: https://github.com/nazar-pc/abundance/pull/288

From a different side [PR 284] improved block APIs with better efficiency using self-referential data structures to
avoid repeated parsing work, especially for cached block headers and to provide. [PR 287] refactored public APIs to make
key block-related data structures `#[non_exhausive]`, which forces users to go through provided constructors, which in
turn allows to simplify APIs that convert to owned data structures since the only way to create non-owned data structure
is going through constructors that ensure proper invariants. It is still currently possible to mess up data structures
after they have been created, which I'll probably fix in the near future too.

[PR 284]: https://github.com/nazar-pc/abundance/pull/284

[PR 287]: https://github.com/nazar-pc/abundance/pull/287

## Upcoming plans

This week I plan to work on MMR, super segment abstraction, look into state management, block execution, probably port
farmer RPC interface so that it is possible to start block production.

Currently, farmer crate depends on now broken GPU plotting implementation crate, so if I'm in the mood I might try to
implement something using CubeCL. Will probably not be very performant initially, but should at least serve as a stopgap
and inform potential API changes while hopefully avoiding coupling to AMD/Nvidia libraries during compile time (need to
look closer into it). If it is really ergonomic to use, might open possibilities to use GPU acceleration more widely in
the node too, not just farmer.

With enough components in place it should become possible to prototype node CLI implementation. With more clarity in
hierarchical consensus, it should be possible to start prototype intermediate and later leaf shard block
production/import.

Basically there are more things left to do than those which were done already, but we're making progress. [Zulip] chat
is the best place to ask questions and discuss future work.

[Zulip]: https://abundance.zulipchat.com/
