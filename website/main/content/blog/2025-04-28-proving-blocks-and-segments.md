---
title: Proving blocks and segments
date: 2025-04-28
draft: false
description: Generating proofs that blocks and segments are part of the global history.
tags: [status-update, consensus]
authors: [adlrocha]
---

We started last week with two PRs that attempted to describe in detail the operation of sharded
archiving ([PR192](https://github.com/nazar-pc/abundance/pull/192)), and the data availability layer
of the system ([PR193](https://github.com/nazar-pc/abundance/pull/193)). When I started writing this
spec, it was meant to be for a broader audience, but we realised after a few rounds of feedback that
the project is still in a really early stage and in constant change, so it would be more efficient
to focus on detailing the parts of the protocol that are currently under-defined instead of trying
to give a deep overview of the overall operation of the protocol from the get-go. The actual goal
behind this protocol specification is to unblock the implementation of a prototype that can help us
gain certainty about the design decisions that we are making, and surface potential blind spots in
the design, and not to have a reference spec (just yet).

<!--more-->

## Making the code the source of truth

We inherit a lot of the mechanics of the protocol from Subspace's operation, and as I was writing
the spec I saw myself repeating many of the details already specified in the
[Subspace specification](https://subspace.github.io/protocol-specs/docs/protocol_specifications).
Even more, as I was writing it, Nazar was making really good progress on the implementation of the
prototype,
[replacing KZG with the new Merkle-proof based archiving](https://github.com/nazar-pc/abundance/pull/175),
[refactoring and simplifying the implementation of different modules](https://github.com/nazar-pc/abundance/pull/177),
and stripping the code base from unnecessary overhead and dependencies, which led to parts of the
spec becoming obsolete quite fast.

With this in mind, we decided to repurpose the spec to make it more maintainable and useful for its
original purpose, allowing us to flesh out the design that can unblock the implementation of a
prototype and iterate on it. Thus, what the spec will become from now on is an outline of all of the
basic sub-protocols and modules of the system. Instead of having a detail spec describing their
operation, they will reference the parts of the code that implement them, making _the code the
source truth for the protocol implementation_. This avoids unnecessary duplication, potential
mismatches between the spec and the implementation, removes a lot of maintenance overhead, and it
enables us to move faster in the design focusing on the parts that are still under-defined. This
will also help us move from code to spec once we are comfortable with the design, as we just need to
follow the reference in this spec outline and translate the implementation code into a spec _"(hey,
we may even be able to delegate this task to an AI in the not-so-distant future)"_.

## Proving blocks and segments belong to the history of the system.

One of those parts of the protocol that were currently under-defined and that were blocking the
implementation, were the proofs to verify that blocks and segments of a shard belong to the global
history of the system. After some feedback iterations, this has been my main focus for the last
week, and I am proud to share that we may have a first candidate design for these.

### Block proofs

Let's start with blocks. If you recall from previous updates, shards from the lower levels of the
hierarchy are periodically committing the new blocks they are creating to their parents. This makes
their history available to the rest of the system before a new segment of shard is created and made
available in the global history maintained by the beacon chain (which make take some time to be
available). These commitments would allow nodes to proof (and verify) that a block belongs to the
history of the shard, and is consistent with the global history of the rest of the shards and the
global history of the system.

The following diagram shows a high-level of how I am thinking about the proofs for blocks. The idea
is to use a
[Merkle Mountain Range (MMR)](https://docs.grin.mw/wiki/chain-state/merkle-mountain-range/) to store
the history of the blocks in the shard, and use references to blocks in the beacon chain to attach
them to the global history in the beacon chain. This allows for the generation of incremental proofs
of inclusion of blocks in the history of the shard, and their consistency with the global history.
Child shards are periodically sending their block headers to their parent. Parent shards keep track
of child shard new blocks, and they build their own view of the history of the shard. New blocks in
shard need to always point to recent blocks in the beacon chain that can be consider final.

Thus, when a network participant wants to prove that an event has happened, and this proof relies on
a block in a child shard being valid, they can generate a proof of inclusion of a shard block by
requesting a MMR proof for the block from the shard's history kept by the parent, and verifying that
is consistent with the history of the parent shard, and that they point to valid blocks in the
beacon chain (see the bottom-right part of the diagram for the high-level steps of the proof
generation/verification).

<p align="center">
<img alt="High-level diagram of block proofs" src="mmr-block-proofs.png"></img>
</p>

### Segment proofs

And what about segments? For segments, the main thing that we want to proof is that a piece is part
of a segment that is in itself part of the global history of the system committed in the beacon
chain. Shards are periodically generating new segments of their local history. Information about new
segments in a child shard are submitted to their parents that are responsible for forwarding them to
the beacon chain. When new segments are committed in the beacon chain (coming from any shard in the
lower-levels, or even from local segments from the beacon chian), they are added to the global
history of the system kept in the beacon chain. Thus, child shard segments leave their shards with a
local index that determines the sequence in which they were generated in the shard, but as soon as
they are committed to the beacon chain, a global history index is assigned to them that determines
their position in the global history of the system.

Finally, a new data structure that we are calling a `super_segment` is created with every new block
in the beacon chain that includes the commitment of new segments to the global history, and is used
to aggregate the commitment of several segments, limiting the amount of information required by
nodes in order to verify that a segment was successfully committed in the beacon chain, and that it
belongs to the global history of the system.

The following diagram describes at a high-level (but with a little more of low-level details) the
operation described in the previous paragraph. A basic description of how to generate the proofs can
also be found in the text box of the bottom-right section of the diagram.

<p align="center">
<img alt="High-level diagram of segment proofs" src="segment_proofs.png"></img>
</p>

## What's next?

Now we need to go from these high-level diagrams to the low-level details of the specific data
structures, information required for the proofs, and were they need to be stored to make it
available for nodes in the system. My week will be focused on writing a set of GH issues that detail
these so I can get feedback from Nazar, and hopefully enable him to start kicking-off their
implementation. But as always, if you have any feedback or suggestions in the meantime, please let
me know. Until next week!
