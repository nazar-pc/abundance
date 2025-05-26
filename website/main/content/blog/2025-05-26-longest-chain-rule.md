---
title: Longest-chain rule and blocks probability of reorganisation
date: 2025-05-26
draft: false
description: How relying on the longest-chain rule can fit all protocol mechanics together
tags: [status-update, consensus]
authors: [adlrocha]
---

I started the week thinking about the mechanics of shard segment commitment into the global history
of the beacon chain. If you recall from previous updates, we already had a pretty good idea of how
the information about child shard segments flow up to the beacon chain, but there were still a few
questions that were really bugging me.

<!--more-->

Mainly:

- When a segment is committed in the beacon chain, how can we inform the corresponding child shard
  that the segment has been added to the global history and assigned a global piece index?
- How can we ensure that a segment is _"final"_ with a really high probability before committing it
  to the global history?
- How do block re-organisations in the beacon chain, or lower level shards, affect the flow
  information up the hierarchy?
- How do we ensure that the information about the shard segments is available and verifiable, and
  that segments are encoded correctly and include the set of blocks that is supposed to have?

Thinking about all these questions led to a realisation that, while obvious in hindsight, is going
to really simplify the design of the protocol and its security analysis moving forward: we can embed
all the core protocol verifications and mechanics into the longest-chain rule (including block shard
submissions, segment verification and commitments, chain re-organisations, challenges and data
availability checks).

With this, all chains in the hierarchy are able to deterministically reason about the validity of
the information included in blocks without having to rely on external mechanisms like fraud proofs
or a dedicated data availability chain (and this is why the design of Bitcoin is so elegant!).

## Impact of chain re-organisations in shard block submissions

Let's illustrate how this new realisation of leveraging the longest-chain for all core protocol
mechanics come to play starting with how it affects the shard block submission process, which is
going to be the main data structure impacted by chain reorganisations. See the image below for
visual aid to the following explanation.

- When, e.g. an intermediate shard proposes a new block `blkA`, this block will have a reference to
  a valid block from the history of the beacon chain.
- `blkA` will be submitted to the intermediate's shard parent, in this case the beacon chain, inside
  an `IntermediateShardBlockInformation` data structure.
- This process will be repeated for every new block in the shard: `blkB`, `blkC`, etc.
- Verifying the validity of a shard block is done by making the regular block validity verification
  and all the additional verifications imposed by the hierarchical consensus like checking that the
  referenced beacon chain block is valid. You can already glimpse here how we are not only embedding
  in the longest chain rule local consensus verifications, but also logic involving the overall
  hierarchical consensus operation.
- So far so good. However, all the shards in the system are running a probabilistic consensus. What
  happens if there is a chain re-organisation and a heaviest chain surfaces replacing the current
  longest-chain? Depending on the shard suffering the re-org, the impact is different. Fortunately,
  all of them can be handled quite gracefully through the longest-chain rule.

  - Upward reorgs, meaning leaf or intermediate shards that suffer a re-org, are easily detected. As
    blocks are being submitted to the parent, the parent is able to identify that there is a
    heaviest chain that needs to replace its current view of the child shard. This requires no
    immediate action from the parent apart from this update of the parent's view of the heaviest
    chain.
  - Reorgs from intermediate shards do not involve any immediate action from its child shards. The
    newest heaviest chain may change the way (and specific blocks) where the information about the
    child shard is being included into the parent chain and propagated to the parent chain, but this
    shouldn't have any additional impact.
  - Finally, beacon chain re-orgs are the only onces that may trigger actions in the lower levels of
    the hierarchy. When there is a heaviest chain in the beacon chain replacing the current longest
    chain, this can trigger a re-org in some shards of the lower levels of the hierarchy that were
    following the previous longest chain, as their beacon chain references for some of their blocks
    may be deemed invalid. This event is also handled gracefully by the longest-chain rule. The
    farmer entitled to create the next block in the child shard after the beacon re-org will build
    the new block on top of the latest block in the child shard with a valid beacon chain block
    reference after the re-org, continuing in this way constructing a valid chain in the shard.

<p align="center">
<img alt="Sketch of core mechanics in block submission and re-org handling" src="reorg-sketch.png">
</p>

## Shard segment commitment and block probability of reorganisation

While handling re-organisations of shard block submissions is somewhat _"cheap"_, this is not the
case for shard segments. Segments committed to the beacon chain become part of the global history,
and will be conveniently archived in farmers plots. Reverting the archival of a segment is an
_"expensive"_ operation that should be avoided at all costs. Consequently, segments should only be
propagated up and committed in the beacon chain when their probability of being reverted or
re-organised is negligible. This is the high-level proposal of how I am thinking about handling
this:

- Every parent chain (intermediate shard and the beacon chain) keeps a view of the current state of
  their child shards through the shard blocks that they are periodically submitting. Through this
  view, they can track of the segments that have been submitted by their children, and in what block
  they were submitted.
- Along with this view of blocks and segments, the parent chain will also keep track of the
  probability of re-organisation for each recent block in the child shard to have a sense of when a
  block can be considered final with high probability, and thus any segments submitted with it.
- Segments are propagated to parents as soon as they are created, but they will only be confirmed
  once the probability of re-org for the block where they were included is below certain threshold
  (e.g 0.1%). Similarly, the beacon chain will only consider segments for inclusion in the global
  history once the probability of re-org for the block where they were included is below 0.1%.
- In parallel, as soon as a segment is created in a child shard, the data availability layer will be
  checking that it is correctly encoded and can submit challenges that the segment is not valid.
- If a block or a segment is challenged because is not available, it can't be verified, or it was
  wrongly encoded, the beacon chain (or its corresponding parent for leaf shards) will pause the
  commitment of new segments and blocks for the shard until a counter-challenge is sent reverting
  the failure situation.
- All of this is flow of information (i.e. block and segment challenges, re-org confirmations etc.)
  is handled through the proposal of new valid blocks submitted from the lower to the upper layers
  of the hierarchy.
- With this, we add a dynamic waiting time for segments that depend on their probability of re-org
  that gives enough time to consider the segments final and to challenge potential misbehaviors and
  mistakes.

To try and clarify the description above, let me share how this would work step-by-step for the
commitment of a segment `s1` for a leaf segment:

1. A farmer in the leaf shard creates a segment `s1` that is included and submitted to the parent as
   part of the `own_segments_root` field of the `LeafShardBlockInformation` data structure that is
   included in `blkA` of its intermediate shard parent. `s1` was created in `blk1` of the leaf
   shard.
2. As soon as the parent sees this new child segment, it includes it in its next
   `IntermediateShardBlockInformation` inside `child_segment_root` included in `blkX` of the beacon
   chain. This makes the leaf segment immediately available in the beacon chain, but it won't be
   included into the global history, and be included in the corresponding super segment, until the
   probability of re-org of all the blocks involved in the submission of the segment, mainly `blk1`,
   `blkA` and `blkX` is over the finality threshold chosen.
3. The intermediate shard is periodically updating its perceived probability of reorg for the leaf's
   shard `blk1` and it will notify the beacon chain in the following
   `IntermediateShardBlockInformation` that the finality threshold has been achieved. Similarly, the
   beacon chain keep track of this same threshold for `blkA`. When both are confirmed as final, the
   leaf segment of that block is committed as part of the global history, assigned a global piece
   index within that history, and made available for archiving.

## Computing the block probability of reorganisation

And many of you may be wondering at this point, but how can we compute the probability of
reorganisation for a block in the first place? Fortunately, there is a lot of literature around this
topic and attempts to objectively measure this metric or related ones for longest-chain protocols
(including the original Bitcoin white paper, with its analysis of its probability of attack):

- [Satoshi Nakamoto's Bitcoin Whitepaper](https://bitcoin.org/bitcoin.pdf)
- ["On Finality in Blockchains" by Anceaume et al.](https://arxiv.org/abs/2012.10172)
- And quantitative approaches to blockchain finality for
  [Markov Chain models](https://link.springer.com/article/10.1007/s00145-016-0259-8), and formal
  analysis of Bitcoin's backbone and for
  [game theory and selfish mining](https://www.cs.cornell.edu/~ie53/publications/selfish-mining.pdf)
  (relevant to miner incentives and reorgs).

From the analyses on these papers, I extracted the following high-level formula to compute the
probability of block reorganisation that we will be able to combine to our needs:

```rust
Prob (num_blocks_replaced) = (2*time_to_first_block_propagation/avg_time_between_blocks)^(num_blocks_replaced)
```

## What's next?

I shared here a pretty high-level overview of all of these new mechanics for block re-orgs and
segments commitments into the global history. This week I'll focus on having a spec-like low-level
step-by-step description of all of this mechanics so they can be reviewed by Nazar for them to
(hopefully) be in a good state to start being eventually prototyped.

In parallel, and as it's been the case for several weeks now, I want to find time to share how I am
imagining the data availability layer to work as part of the core protocol (not as an independent
layer as it is the case in other projects), and how I am planning to integrate its operation into
the longest-chain rule and the core protocol. As always, any feedback, ideas or suggestions? Hit me
up!
