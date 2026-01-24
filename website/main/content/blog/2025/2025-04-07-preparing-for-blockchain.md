---
title: Preparing for blockchain
date: 2025-04-07
draft: false
description: Initial transaction pool implementation and Subspace codebase import
tags: [ status-update ]
authors: [ nazar-pc ]
---

The majority of last week I spent tinkering with Subspace codebase after importing it here in preparation for building
an actual blockchain.

<!--more-->

As mentioned in previous updates, transaction was the next logical step and after some preparation in [PR 149] initial
version landed in [PR 151]. It is quite basic and will likely require significant changes before used in an actual
blockchain implementation, but there are just too many unknowns for now.

[PR 149]: https://github.com/nazar-pc/abundance/pull/149

[PR 151]: https://github.com/nazar-pc/abundance/pull/151

On a similar note, [PR 156] added an initial version of the native token, which will be important for charging
transaction fees. It is relatively simple for now, but in the spirit of what it will eventually end up being.

[PR 156]: https://github.com/nazar-pc/abundance/pull/156

With that, [Subspace codebase] was imported in [PR 152]. This initial PR removed a bunch of features of the original
codebase that will not be applicable here (domains, votes, etc.). There were many follow-up PRs that removed/refactored
more things to reduce the feature set and reduce Substratisms from the codebase ([PR 154], [PR 155], [PR 157], [PR 158],
[PR 159]). As the result the blocks can still be produced, but RPC is mostly gone (only supports what farmer needs) and
many of the secondary features are gone too.

[Subspace codebase]: https://github.com/autonomys/subspace

[PR 152]: https://github.com/nazar-pc/abundance/pull/152

[PR 154]: https://github.com/nazar-pc/abundance/pull/154

[PR 155]: https://github.com/nazar-pc/abundance/pull/155

[PR 157]: https://github.com/nazar-pc/abundance/pull/157

[PR 158]: https://github.com/nazar-pc/abundance/pull/158

[PR 159]: https://github.com/nazar-pc/abundance/pull/159

## Blockchains are hard

Now the challenge is figuring out where to go from there. Substrate is great because it gives a framework to work with,
where an initial version of the blockchain may not be exactly what you want, but at least you have something working all
the time. When bootstrapping from the ground up, it is easy to get lost since there are countless "chicken and egg"
kinds of problems.

The plan right now is to first extract core consensus logic contained in the runtime into system contracts. After that
to somehow get enough of node infrastructure up to get a single-node blockchain without P2P networking producing blocks.

P2P networking for blockchain will be a major effort since while Distributed Storage Network (DSN) in Subspace is
standalone, block and transaction propagation, PoT gossips are all built on top of Substrate and were never migrated off
of it. Given our sharding requirements, it is likely that this (originally) DSN networking stack will have to evolve to
support more things, but that is a big challenge.

So yeah, blockchains are hard to pull of the ground, but when building something truly different, you sometimes have to.
Thankfully, some lessons were learned, and the logic already written and audited in Subspace codebase can be partially
reused to speed up the process (hopefully).

## Upcoming plans

It is hard to predict how such open-ended projects will go, but one way or another I'll be working on building the
blockchain to allow us experimenting with sharding that Alfonso is designing. One thing for certain is that you can
expect regular progress updates along the way.

Until next time 👋
