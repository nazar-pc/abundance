---
title: The data availability problem
date: 2025-04-14
draft: false
description: How to ensure that blocks and segments are available in a shard when you need them.
tags: [ status-update, consensus ]
authors: [ adlrocha ]
---

This week has been another good week of progress. I finally have a good idea of how shard archiving
should work in the happy path, and I've started writing a low-level spec for it (that I am hoping to
push to this repo soon). Unfortunately, there is still a slight gap in the spec that we need to fill
before we can move forward: the _data availability problem_.

<!--more-->

## Current state of sharded archiving design

High-level, this is how the end-to-end of sharded archiving currently looks like:

- _Block Creation (L2 Child):_ New blocks (`blk1`, `blk2`, etc.) are created normally in the child
  shard. As soon as they are created, their headers (or block hashes) are immediately committed to
  the parent chain (L1 child).
- _Parent Shard Tracking (L1 Child):_ The parent shard keeps track in a block buffer of all the
  headers (or hashes) from the child shard blocks that have been committed, creating in this way an
  intermediate block history of all its child shards until a new segment from the shard arrives.
- _Block History Management (L2 Child):_ When the history buffer in a shard accumulates enough
  blocks, a new segment is created in the shard. This triggers the creation of a super segment
  (`super_segment_L2_1`) which includes information about all segments and commitments added to the
  history and that is correspondingly submitted to the parent for commitment. It is worth nothing
  that super segment do not propagate the whole segment, but aggregate commitments for the segments
  that can be used to verify them in the upper layers of the system.
- _Recursive operation (L1 Child):_ The protocol is recursive, so in the same way that the L1's
  child shard was committing blocks to the parent as they were being created, it will do the same
  with its own parent (the beacon chain).
- _Super-Segment Commitment (L1 Child):_ When a super-segment from a child (`super_segment_L2_1`) is
  committed, it is added to the shard's history buffer along with other local segments that may have
  been created, triggering the clearing of the block buffer for child shards. Once a segment is
  committed in the parent, there is no need to keep raw blocks in the buffer anymore as they are
  explicitly available in the committed segments.
- _Beacon Chain Integration (L1 Child):_ The beacon chain receives the blocks and segments from the
  immediate shards below (L1 shards), committing them to its chain (as any other regular parent
  shard in the system that has immediate child shards below).
- _Super-Segment Commitment (L1 Child):_ When a super-segment from a child (`super_segment_L1_1`) is
  committed, it's added to the shard's history buffer, triggering the clearing of the block buffer.
- _Segment History (L1 Child):_ Segments in a child shard are created as super-segments, committed
  with a list of shard and child super-segments created up to that point.
- _Beacon Chain Role:_ The beacon chain commits all super-segments (`segment_commitment_L1_1`,
  `super_segment_BC_1`, etc.), maintaining the whole system's historical buffer and creating a
  unified history.

<p align="center">
<img alt="High-level stages and primitives for sharded archiving" src="2025-04-14-sharded-archiving.png">
</p>

## How many shards can we afford if we are committing every block?

You see in the description from the previous section that one of the key design changes since last
week is that we will be committing every block to the parent to have part of the history of child
shards as raw blocks until full segments for the shard are created.

Assuming that we dedicate only a 10% of the full size of a block for the commitment of child blocks,
let's do a back-of-the-envelope calculation of the total number of child shards that we can afford.
Assuming:

- `BLOCK_SIZE = 4MiB`
- `BLOCK_TIME = 6 seconds`
- `BLOCK_HEADER = 224B`

```
type BlockHeader (260B) {
    number (8B)
    extrinsic_root (32B)
    state_root (32B)
    parent_hash (32B)
    shard_id (4B)
    hash (32B)
    solution (120B)
}
```

And that every 6 seconds all the shards in the lower level submit a block at the same time (this is
the average case scenario), we can support the following number of shards in the lower level:

```
NUM_SHARDS = (0.10 * BLOCK_SIZE) / BLOCK_HEADER =~ 153 SHARDS
```

This assumes that the block time of shards is also 6 seconds and that the average case scenario
happens where all shards submit their blocks at the same time on a block.

But maybe committing full block headers is just too much, what happens if we just commit the block
(32B) hash and the shard id (4B)? Then things look a bit better, as we can get to the order of the
thousand shards under each parent:

```
NUM_SHARDS ~= 1111 SHARDS
```

If we come back to our target of around 1M shards discussed last week, these numbers mean that with
just two layers of shards (and the beacon chain in the root), we would be able to achieve the
desired scale for the system.

## Enter the data availability problem

So far so good, we haven't found any big blockers to the design, right? Well, we still have an
elephant in the room that we need to address. While we are only committing the block headers (or
hashes) to the parent, and super segments (that include an aggregate commitment for the segments in
a shard) we need the raw blocks and segments to archive the history. These commitments can be used
for verification, but we still need the full blocks and segments to be available in the shard when
we need them. This is the data availability problem.

Fortunately, this is a problem that has been around for a while in the space (especially in the
scope of Layer 2s and sharded architectures). There is a lot of literature and working
implementations of projects dedicated exclusively to solving this data availability problem for
other networks like [Celestia](https://docs.celestia.org/) or
[Avail](https://docs.availproject.org).

Roughly speaking, all the protocols proposed to solve data availability share common primitives: (i)
chunking of blobs into smaller pieces that can be distributed throughout the network, (ii) erasure
coding to ensure that the data can be reconstructed even if some pieces are missing, (iii) the use
of vector commitments to generate proofs of possession of the data, and (iv) random sampling to
verify that the data is available without having to download the entire blob, and to force holders
of data to create availability proofs of the stored data.

## Next steps

This week I will be focusing on trying to specify our solution for the data availability problem so
I can have a first draft of the end-to-end spec for sharded archiving. High-level, how I am thinking
about this is as follows:

- When segments are created, they are already in an amenable format to create data availability
  proofs (with erasure coding and vector commitments), but block aren't. The blocks included in the
  block buffer will thus have to be chunked and encoded before being committed to the parent to make
  them more friendly to generating availability proofs.
- Instead of forcing the sampling rate by protocol, nodes will be allowed to sample the availability
  of segments at any slot. In order to access the pieces they need to create their plots, farmers
  need the pieces of these segments to be available, so they are incentivised to do it periodically
  to ensure that they are available when needed with a high-probability (otherwise they won't be
  able to create their plot until the piece is recovered).
- When a piece is sampled and not available, the reporting node will have to submit a transaction
  reporting the unavailability to the beacon chain.

There are small details that need to flesh out from the sketch above, and I am planning to go
through many of the data availability protocols to see if we can borrow some of their ideas. Really
looking forward to sharing more of my progress next week.
