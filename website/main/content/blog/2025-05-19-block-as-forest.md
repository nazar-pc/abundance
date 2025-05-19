---
title: The Unblocker - Blocks as a forest of trees
date: 2025-05-19
draft: false
description: Unblocking progress through the design of blocks as a forest of trees.
tags: [status-update, consensus]
authors: [adlrocha]
---

I am pretty happy with the progress this week. Funnily, the main culprit for all of this progress
has been the the work that Nazar has been doing on the definition of the block structure for our
hierarchical consensus. I'll let Nazar dig deeper into what he's been doing here, but let me share
in this post what this block structure entails, and how this has unblock several lines of work, and
solved many issues for me.

<!--more-->

## Cracking the first piece of the puzzle: blocks as trees.

If you recall from previous posts, we already had a pretty good sense of what is going to be the
lifecycle of blocks in the system: from being created in a shard, to being submitted and committed
to its parent, and how this information is implicitly propagated up and can be used to generate
proofs about the history of a shard. Unfortunately, we were missing one piece to this puzzle that
was dragging progress: making a decision of what would be the block structure, as this would
influence the design of other mechanics in the system. After a few back-and-forths, it looks like
Nazar has cracked it down on the PRs referenced by
[this discussion](https://abundance.zulipchat.com/#narrow/channel/495788-research/topic/Mechanics.20of.20child.20block.20submission.20to.20parent.20chain/near/518994821).

The key _"innovation"_ that this design for blocks has is that all the block information (including
the header) is structured as trees. This has a lot of interesting properties, as it allows not only
to leverage information from the header to generate proofs about the information included in the
body, but also to generate proofs about the header itself without having to provide the whole
header. This is a pretty powerful property for a hierarchical network architecture like the one we
have, because it enables the generation of recursive proofs involving information from blocks (and
implicitly state) belonging to different shards.

Here's a high-level sketch from one of our discussion syncs that briefly depicts what this block as
trees would look like (although I would recommend you to check the code to get a full sense of what
is going on under the hood):

<p align="center">
<img alt="High-level sketch of the tree structure for blocks" src="block-header.png">
</p>

## Everything is a tree!

Nazar already alluded to this in his post
[Everything is a tree!](https://abundance.build/blog/2025-04-14-trees-everywhere/), but now it has
become even clearer. By leveraging tree-based data structures are able to logically organise the
information scattered in different shards as if they were part of the same network. These trees will
become _"the verifiable glue"_ of the system.

Let me try to back my claim with an example: in order for a shard block to be valid, it needs to
reference a valid block in the beacon chain that is in the past but builds upon the last beacon
chain reference included by its parent block. Looking at the image below, shard block `s3` in the
figure references block `b4` of the beacon chain, that builds upon `s3`'s parent `s2` beacon chain
reference, `b2`. So shard blocks are not only implicitly included in the headers of their parents,
but also cross-link the history of the beacon chain, linking in this way the histories of all the
shards in the system. What this enables is that, assuming their availability, we can walk the
hierarchy from a block to the beacon chain, to any block in a leaf shard of the system, and
consequently generate (and verify) proofs of it.

Basing the system on the generation and verification of recursive tree proofs that can be combined
to create more complex behaviors allows us to have a simple and elegant way to reason about the
system as a whole. I personally think that one of the reason for Bitcoin becoming so successful is
because it is based on really simple rules that are easy to understand and reason about (simplifying
the analyses of its security, performance, and operation). And this is what we should aim to
achieve.

<p align="center">
<img alt="Shard blocks referencing the beacon chain" src="beacon_chain_ref.png">
</p>

## What this block definition unblocked?

What this block definition has enabled for me is:

- The ability to start thinking about system re-orgs, forks, and syncing shards from scratch. After
  some thinking and a brief discussions with Nazar, it turns out that we may leverage many of the
  mechanics that Subspace currently has in place and leverage them almost unchanged for our case.
  Some notes about this can already be found in
  [this meeting notes](https://abundance.zulipchat.com/#narrow/channel/502084-meeting-notes/topic/2025-05-14/with/518061033).
  I will, however, try to get a few sections written about how I am imagining this to work on
  [this discussion](https://github.com/nazar-pc/abundance/pull/220)
- While discussing about the different fields that the header would include, we surfaced interesting
  questions about how the difficulty adjustment for the different shards, along with block rewards,
  can be leveraged to assign rational farmers to shards in a way that load balances the system and
  improve the security of weak networks (trying to address the problem of power dilution). More
  context about these can be found in this
  [meeting notes](https://abundance.zulipchat.com/#narrow/channel/502084-meeting-notes/topic/2025-05-16/with/518513313)
  and this
  [Zulip discussion](https://abundance.zulipchat.com/#narrow/channel/495788-research/topic/Shards.20dynamic.20difficulty.20adjustment/with/518889341).
- Finally (and this is what I want my focus to be next week), with the basic structure designed, we
  can start defining how will the information about shard segments be propagated to the beacon chain
  to commit them in the global history and into super segments, and the basic information that super
  segments should include so light clients can verify the history of a shard seamlessly.

## The essential need for a data availability layer

As always, not all that glitters is gold. This forest of trees is great to recursively proof
information stored anywhere in the hierarchy, but as briefly mentioned above, this only works as
long as we can assume the information to be available somewhere. A verifiable reference to some
piece of information it is worth nothing if we can't access the underlying content. This is why it
has become clearer than ever this week the need for a data availability layer for a system like this
to work. Ideally, this data availability layer won't be an orthogonal system like it is the case in
other L1 and L2 projects, where they either leverage systems like Celestia or Avail to handle data
availability, or build their own independent data availability system, like Espresso.

In our case, data availability will be implemented and integrated into the consensus protocol
itself, to prevent as much as possible the need of fraud proofs and the implementation of special
logic to cover corner cases. Fortunately, data availability is a field that it is still being
actively researched, and Nazar already found pointers to interesting and
[relevant work](https://abundance.zulipchat.com/#narrow/channel/495788-research/topic/ZODA.3A.20Zero-Overhead.20Data.20Availability/with/518435332).
While not immediately applicable, the concept introduced by the ZODA (Zero-Overhead Data
Availability) paper of the "Accidental Computer" may allow us to embed different types of proofs
within the data availability layer. In our case, we not only want to prove that a block (or a
segment) is available in the system, but also that its encoding is correct, that the information
included in them is valid. Reading through this work and thinking a little bit about this problem
has also occupied a bit of my week.

## Next steps

With all of this, my focus this week will be on organising all of the ideas surfaced from last
week's work:

- By writing down into the relevant discussion PR all the outcomes from our research and
  conversations.
- Coming up with a draft design of segment commitment and the structure of super segment leveraging
  the current structure of blocks.
- And time permitting, continue researching the design of our data availability layer.

And as always, any feedback, suggestions, or comments, please let me know so I can improve my work.
Cheers!
