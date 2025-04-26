---
title: Blockchain as a library
date: 2025-04-26
draft: false
description: What tradeoffs can we make when designing a blockchain?
tags: [ ]
authors: [ nazar-pc ]
---

Most blockchain implementations are pieces of software that include the logic to support many different roles/features,
possibly all at once: bootstrap node, block producer, RPC node, archival node, light client, etc. That is one way to do
it, but one thing I learned over the years is that you can do a lot of interesting optimizations if you can apply
additional constraints during the design phase.

So why is basically everyone trying to combine all into one? Let's consider different roles separately first and see
what is special or interesting about them.

<!--more-->

## Bootstrap node

Bootstrap node is a crucial role of P2P networks, you have to know an existing node in the network to join and software
usually comes with a set of bootstrap nodes preconfigured just for this. One can use their own existing node for this or
ask a friend, but from my experience, most people don't and expect things to work out of the box without extra
configuration.

What does the bootstrap node need? Generally just networking and as little bandwidth as possible so it can sustain a lot
of connections. What it certainly doesn't need is running a fully-featured blockchain node, yet this is currently the
only way to run [Substrate]-based node and that can [easily cause issues], especially if some parts of the node are
inefficient/unoptimized.

[Substrate]: https://github.com/paritytech/polkadot-sdk/tree/master/substrate

[easily cause issues]: https://github.com/paritytech/polkadot-sdk/issues/6848

## Block authoring node

Block producers in contrast to bootstrap nodes don't need as much network connectivity, but they should be able to
follow the chain, maintain transaction pool and create blocks when they have a chance according to the consensus
mechanism rules. This should be done with as little latency as possibly (both in networking stack and execution) since
any delays can reduce the rewards. There is no need to store or query deep history/state, and reorgs are typically only
a few blocks deep. It doesn't really need to know or do much beyond checking that blocks follow consensus rules and
author the next block occasionally.

## RPC node

An RPC node can be used to query blockchain information by browser extension and other tooling, sometimes (often?) used
to index the blockchain to build a block explorer (although inefficiently). In contrast to block producer, it is
important to query blocks and transactions, capture various events happening on a blockchain and associate them with
user transactions (e.g., to display transaction confirmation in a wallet). Depending on use case delays in block import
may be less important since user may want to wait a few blocks for transaction to be confirmed anyway, so a second here
or there doesn't matter as much.

## Archival node

Archival nodes are often combined with RPC nodes to be able to query deeper history/state. However, in many blockchains
they are also important for the ability to sync from genesis, which is generally considered to be the most secure
(although inefficient) way to sync a blockchain node. In contrast to block authoring, there is no rush to import blocks
ASAP at all. Archival node can even afford to wait for block to be reasonably deeply confirmed before even bothering to
import it, which will barely compromise its functionality.

## Light client

Light client is the most special out of all mentioned so far. The goal of the light client to be "light" in terms of
CPU, storage and memory requirements. In some cases, it may run in a smart contract, a very constrained environment!
More likely, though, it may be a component of a wallet application, making it somewhat independent of RPC nodes and more
secure and private than simply querying a centralized server. Light client typically doesn't care about transactions at
large, only monitoring a small subset of them that are of interest, otherwise simply verifying block headers to stay in
sync with the network.

## What does this tell us?

Well, as we can see, different kinds of nodes have sometimes conflicting requirements, so trying to design a single
piece of software that serves all (or most) of them will inevitably lead to inefficiencies.

For example, block author can probably afford to store recent data in memory and prune most of the historical data
rather quickly, which leads to interesting performance optimization opportunities.

The archival RPC node needs to store a lot of historical information, and it needs to be efficiently retrievable, but it
is not practical to fit it all into memory. Moreover, while the majority of requests will likely hit information from
recent blocks, there might be older blocks of historical significance that might be queried relatively frequently, so
caching might be very helpful to respond to queries quickly.

Light clients are a completely different beast, often requiring a separate reimplementation with a different set of
tradeoffs like [smoldot].

[smoldot]: https://github.com/smol-dot/smoldot

## Real world

The real world is even messier than the above description. When hitting a public RPC endpoint, you may assume you're
connecting an RPC node described above. However, due to the need to serve a large number of clients, rate limit (and
sometimes charge) connections/requests, you're likely connecting to a complex geo-distributed cluster composed of many
different pieces of custom software. It just happens to provide an identically looking RPC endpoint, but internals are
completely different.

If you tried to index a blockchain through queries to RPC endpoint of an archival node, you may have noticed how
painfully slow it could sometimes be, with even JSON encoding/decoding alone taking a non-negligible amount of compute.
This doesn't even account for the fact that blockchain indexing may require more detailed information than "standard"
RPC can return.

## Make it a collection of libraries instead

What if there wasn't a single implementation for all use cases? What if instead there was a collection of libraries,
which can be composed into larger apps in a way that makes most sense instead?

This is to some degree what [Substrate] allows to do. In fact, [Subspace] does use it as a library. In contrast to most
Substrate-based chains, it doesn't use Substrate's CLI interface at all. This allows for Subspace node implementation
itself to be a library, which together with a farmer component was wrapped into a desktop application in the past, most
recently into [Space Acres]. But even then, Substrate is not as flexible or at least not as easily usable for building
blockchain client implementations with completely different requirements. It was designed to build different kinds of
blockchains and serve that role successfully. However, it is often very difficult or even impossible to swap important
parts and challenging to hyper-optimize due to a lot of generic APIs that any custom implementation must satisfy,
whether it needs them or not (ask me how I know).

[Subspace]: https://github.com/autonomys/subspace

[Space Acres]: https://github.com/autonomys/space-acres

Bootstrap node implementation could pick just the networking stack and configure it in a way that supports multiple
clients.

Block producer could include a hyper-optimized execution environment with a blazing fast database that doesn't hit the
disk very often and doesn't need a lot of disk space. RPC node may not be needed at all.

Indexing software can embed blockchain as a library, bypassing constraints and inefficiencies of an RPC interface.
Extensible VM might allow transaction introspection with arbitrary precision (down to individual contract calls or even
specific instructions) that requires much more processing power without affecting other kinds of clients. And the
results can be written into a high-performance distributed database like ScyllaDB where they will actually live for
production needs, possibly without maintaining historical information on the node itself.

Having [Proof-of-Archival-Storage] consensus means archival nodes are not needed to sync from genesis at all.

[Proof-of-Archival-Storage]: https://academy.autonomys.xyz/subspace-protocol/consensus

Light client could combine core logic that verifies the consensus, combine it with a custom networking stack that
compiles to WASM and lives in a browser extension.

## This is what we do

We're not building one-size-fits-all massive blockchain node implementation, but rather a collection of libraries that
can be combined in various ways to satisfy a diverse set of requirements. The ability to reduce the design space for
each unique use case allows writing better software. Software, which works faster, needs fewer resources and delights
its users.

The reference implementation will offer an optimized block authoring node and likely a desktop application
like [Space Acres] for the same purpose. There should be examples of other nodes/clients and abstractions to make them
possible, but it'll require a community/ecosystem effort to produce high-quality software that does things that people
need.

A lot of software engineering is about picking the right set of tradeoffs, and I believe this is the way to go here. If
you agree or disagree and would like to discuss it, join our [Zulip], I'll be happy to chat about it.

[Zulip]: https://abundance.zulipchat.com/
