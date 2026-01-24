---
title: Async transaction processing
date: 2025-08-08
draft: false
description: Latest progress and thinking about the way transactions will be processed in the future
tags: [ status-update ]
authors: [ nazar-pc ]
---

This week I continued working with the client database and integrating it closer with the rest of the node. A key
integration point that was missing completely and still not implemented was state management. The core parts of the
consensus do not involve state management, but transaction processing will. So I was considering various ways to process
transactions and came up with an idea I'll be pursuing that should work nicely, but is also a bit unlike most of the
blockchains out there.

<!--more-->

## Block processing

As a recap, the way blocks are processed in the current implementation is split into several parts, which can be done
concurrently. More specifically, as long as parent block headers are available, child block consensus verification can
already proceed in parallel, but results will be discarded if the parent block happens to be invalid. This does waste a
bit of compute in the worst-case, but massively improves performance otherwise.

Things are a bit different when it comes to state, though. The current implementation has "TODO" in place of
implementation, so I had to think again how to approach it. State is much more involved than just block headers, so it
is unrealistic to expect that a bunch of blocks worth of state will be downloaded during sync (though it is a
possibility). At the same time, it would be really nice to have as much parallelism as possible here as well.

Another complication is that in most traditional blockchains block can only be propagated through the network once it
was fully imported. This substantially increases the amount of time it takes to propagate a block and increases the
probability of short-term forks.

## Async transaction processing

I decided that a good compromise would be to split state update logically into two parts: system contracts are processed
and transactions are checked for legality as part of block import, while transaction execution happens separately.

The key challenge and design constraint most blockchains face is that transactions are inherently untrusted and consume
variable amounts of resources. Often, transactions specify much higher gas limit than they actually consume, which makes
it harder to estimate how many of which transactions fit into the block. What makes things worse is that transactions
often end up modifying the same contract and need to be executed sequentially. All this is not very friendly for
parallelism and high performance.

The concurrent updates are addressed to a large degree by a novel state organization, but the rest of the issues
remained unresolved until recently.

I analyzed various common reasons for why transactions are consuming less gas than the specified limit and why it is a
problem. It ends up being a complex mix of various factors like business logic, non-deterministic storage access,
difficulty in gas estimation, etc. I came to the conclusion that with a more powerful RISC-V based VM and less expensive
transactions it might actually be okay to always consume the gas limit, avoiding refunds completely (storage was
non-refundable already). If we can do that, then it should be possible to pack a full block of transactions even without
executing them!

Here is the proposed workflow:

* Block builder calls system contracts and updates their state
* Block builder runs stateless verification for all transactions and charges a full transaction fee from each that was
  verified successfully
    * Note that this implies users should not send multiple transactions with e.g., the same nonce, which is a bit
      awkward design-wise, but might be acceptable since transactions are mortal and multiple nonces can be supported if
      necessary on the contract level
    * This can be done 100% in parallel, especially with fee subtraction being a cumulative operation
* State root is computed with the result of those two operations
* Block is sent to the rest of the network
* Block import on any node (including block builder) imports the block and starts executing transactions in it, likely
  in parallel
* Creation or import of the next block must wait for results of the transaction execution from the parent block with
  another state root (post-transaction execution) included in it

Essentially, transactions are executed while the block is being propagated through the network and while the solution
for the next block is being computed. This allows for almost a complete block time worth of compute for transactions
rather than a small fraction of it. This kind of pipelining should positively affect both throughput and latency.

Note that the beacon chain and intermediate shards do not have user transactions, so the workflow there will be
simplified.

## Reducing MEV

MEV often comes to mind when talking about DeFi transactions, which is basically the ability to extract additional value
by manipulating what transactions are included and in which order. I think PoT can come to the rescue here as well,
significantly reducing the opportunity.

PoT is already used in Subspace to delay a block proposal by a few seconds, ensuring all farmers, regardless of how fast
or slow they are, have sufficient time to prove they have a solution to the consensus puzzle.

A similar mechanism can be used to delay transaction execution. Transactions are included in the block in
deterministically sorted order when the block is built and fees are charged. However, nodes will have to wait one more
slot after that before they know the shuffling seed that determines the execution order of those transactions. Of
course, some farmers may choose to wait and manipulate the order. However, delaying the announcement of the block means
they will be at a disadvantage compared to potential blocks that were produced at around the same time (natural forks).

Right now I think a single slot might suffice. However, it is possible to increase it to several slots with the tradeoff
being that less time will remain before the next block is created, so the amount of compute (time) allowed will have to
be reduced.

## Other updates

That was all I wanted to say about transactions for now, but there were other things I worked on this past week.

[PR 352] was a big cleanup for the client database to make it more extensible in the future. [PR 354] made another step
towards state management by offering API to persist state of system contracts (as described above), which nodes will
have to store. This is in contrast to the state of other contracts, which they are not responsible to store.

[PR 352]: https://github.com/nazar-pc/abundance/pull/352

[PR 354]: https://github.com/nazar-pc/abundance/pull/354

I also spent some more time on optimizations, this time again on balanced Merkle Tree. Last time I mentioned faster
balanced Merkle Tree construction implemented in [PR 345], this time the same kind of optimization was applied to the
root-only computation in [PR 350].

[PR 345]: https://github.com/nazar-pc/abundance/pull/345

[PR 350]: https://github.com/nazar-pc/abundance/pull/350

Both of these APIs are used in the archiving, so I decided to benchmark it:

```
Before:
65536/balanced/compute-root-only
                        time:   [4.0835 ms 4.0836 ms 4.0841 ms]
After:
65536/balanced/compute-root-only
                        time:   [555.84 µs 556.90 µs 557.80 µs]

Before:
segment-archiving-whole-segment
                        time:   [1.8868 s 1.8892 s 1.8914 s]
After:
segment-archiving-whole-segment
                        time:   [950.64 ms 957.24 ms 962.52 ms]
```

Very substantial improvement as expected, though there are still opportunities for better performance with both Merkle
Tree and erasure coding.

As an interesting reference point, I re-benchmarked Subspace implementation that uses KZG:

```
segment-archiving-whole-segment
                        time:   [39.370 s 39.370 s 39.370 s]
```

Wow, it is a whopping 41x slower and we're not done yet! Both are single-threaded. This is why we had to parallelize it
and implement an incremental archiving solution to smooth out the cost over a longer period of time. But it was still
painful during sync from genesis or when a segment needed to be reconstructed, even on 32C64T CPU it was a noticeable
slowdown when this happened.

## Upcoming plans

One of the key things to achieve in the next few weeks is to get state management to the point when we can execute
system contracts during block building and import. I'm starting with the beacon chain, so transactions will come later.
This is one of the key pieces needed for basic single-node node block production to work.

Work on GPU-accelerated plotting, publishing of crates to crates.io and other miscellaneous things are also still on the
table.

I think this approach to transaction processing might be a bit controversial, so I'm open for any kind of feedback
on [Zulip], please join if you're interested 🙂

[Zulip]: https://abundance.zulipchat.com/
