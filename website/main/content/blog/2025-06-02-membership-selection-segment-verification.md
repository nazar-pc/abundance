---
title: Membership selection and segment verification
date: 2025-06-02
draft: false
description: Leveraging farmer membership shuffling to verify shard segments.
tags: [status-update, consensus]
authors: [adlrocha]
---

This week has been another of those weeks where I am pretty happy with the progress made. The
highlights of the week are the following: (i) we now have a pretty good sense of the end-to-end
operation to commit shard segments into the global history of the beacon chain (as described in the
discussion of [PR267](https://github.com/nazar-pc/abundance/pull/267)); (ii) and Nazar had an idea
to tackle the verification of the availability and correctness of shard segments without requiring
an independent data availability mechanism, by leveraging the longest-chain rule and farmers
membership allocation (which was an issue that was really bugging me). Let's jump into the details
of these two topics.

<!--more-->

## Detailing the flow of information between shards and the beacon chain

If we look at the current design of the Subspace consensus protocol, it is composed of three main
stages:

- Farming, which is responsible for the proposal of new blocks.
- Archiving, which takes care of preparing the history of the blockchain for archival through the
  creation of segments.
- Plotting, which is responsible for actually archiving the history by aggregating segments from the
  history into plots. These plots are used as the sybil-resistant mechanism of the protocol, and
  they determine the next farmer entitled to propose a new block in every slot.

In order to propose new blocks, we first need to have segments created and archived into plots so
that we can determine farmers'eligibility based on their allocated space. This is why, once figured
out the high-level architecture of a hierarchical protocol like ours, the first mechanism that we
started designing was the archiving process, so we could build upon it for the rest of the stages of
the protocol.

The last few weeks, Nazar has done an amazing job implementing the basic data structures and the
"pipes" that enable the flow of information from shards to the beacon chain (mainly through the
`ShardBlockInformation` data structures that we've been discussing in previous updates). Building
upon that and with the latest realisation that we may be able to embed all core protocol
verifications and base the security guarantees as part of the longest-chain rule, in
[PR267](https://github.com/nazar-pc/abundance/pull/267)) I re-wrote the end-to-end steps that
describe how to make segments available in the beacon chain and committed in the global history so
that they can be archived into plots.

Unfortunately, a missing piece in the current design is the verification of the availability and
correctness of shard segments before they can be archived. Initially, I thought that to achieve this
we would necessarily need an orthogonal data availability mechanism that periodically samples shard
segments, verify their correctness, and report any issues to the beacon chain. However, this would
introduce additional complexity and overhead to the protocol, which we wanted to avoid.

Luckily, Nazar had a brilliant idea to tackle this problem by leveraging the re-shuffling of farmers
across shards. Every farmer is assigned to a new shard (or a set of them), they may need to sync
with the latest state of that shard, and we can potentially leverage that work that it already need
to do to verify recent segments and report any misbehaviors to the beacon chain. This, however,
means that before we can detail the process of shard segment verification we need to make quick
detour on our design process to get a better sense of how we want farmer membership to work in the
system.

## Farmer membership allocation for segment verification

The main goal of the membership selection mechanism is to ensure: (i) the load balancing of "power"
(plotted space) across all shards, maintaining an equally high and consistent level of security for
each shard, and preventing potential attacks over shards due to the membership stickiness of farmers
to specific shards that could lead to collusion, or the dilution of power due to farmer churn.

For this, we will consider plots as the basic unit of membership, and we will design a membership
selection mechanism that allows farmers to be assigned to shards based on their plotted space, while
also ensuring that the membership is dynamic and can adapt to changes in the network conditions
(e.g. farmer churn, changes in the total plotted space, etc.).

For this, we need to create a deterministic and verifiable mapping between a plot's unique identity
and its assigned shard. We need to prevent malicious actors from gaming the system by
pre-calculating shard assignments or attempting to concentrate plots in a single shard. Ideally,
this mechanism should be oblivious, so even light clients are able to verify the membership for a
specific epoch without requiring the full list of members or any external information.

Here's a high-level idea of how this allocation mechanism could work, and that I am hoping to get
drafted in more detail the next few days:

- What determines if a farmer needs to sync with a shard is their plots. Plots are the basic unit of
  membership, and they are assigned to shards based on a deterministic mapping.
- Each plot has a unique identity that depends on the farmer's public key, the sector index, the
  global history size, and a unique public key that farmers will generate for each plot (they may
  choose to re-use their own public key for this).
- Each `MEMBERSHIP_RESHUFFLE_INTERVAL` which is a protocol parameter that determines the number of
  slots between membership re-shuffles, farmers will draw the PoT randomness for that slot to
  determine locally the new allocation for their slots.
- This new membership doesn't come to effect immediately, there is a
  `NEW_MEMBERSHIP_WARMUP_INTERVAL` also specified in number of slots that is the interval that
  farmers have to prepare for their new allocation (by syncing with the shard, verifying the most
  recent segments, and any other operation that may be required).
- All this protocol parameters will be determined by the security robustness of the protocol against
  the potential attacks described above, and the technical limitations in terms of warmup
  requirements and overhead of frequent re-shuffles. I already have a sketch for the mathematical
  model that will help us analyse this, and in [this discussion][dynamic-difficulty-discussion]
  Nazar has already shared some number of the technical requirements that we need to consider in
  terms of syncing times and plotting performance.
- Once the new farmer membership comes to effect, farmers will only be allowed to propose blocks in
  their assigned shard, and they will need to prove their membership by signing the new block with
  the public key used to derive the identity of the assigned plot.

There are still a few low-level details that need to be fleshed out here, but once we agree on the
basic operation of the membership selection mechanism, we can start detailing the verification of
segments. Even more, I am thinking that we may be able to implement the membership selection in a
way that allows us to roll-out a simple allocation protocol (e.g. an homogenous epoch-based
re-shuffling as the one described above), that is progressively improved into more complex
space-weighted assignments that introduce improvements to the simple approach.

## Some reading and what's next?

This week I also came across the whitepaper of Solana's new consensus protocol: Apenglow. I spent
some time reading it and thinking about how the ideas presented in it can be applied to our design
(you can follow the [this discussion][apenglow-discussion] with pointers to the announcement, the
paper and the implementation code). My feeling is that their proposal prioritises block throughput
and finality over security, presenting a pretty relaxed security model that tolerates 20% of the
stake being malicious, and an additional 20% of it being unavailable (crash-tolerant). It is
interesting how they leverage error correction to disseminate blocks (and the underlying votes) in a
way that a subset of shreds are enough to validate a block parallelising as much as possible the
proposal of blocks. The paper includes interesting ideas, but considering a security and trust model
that is a bit too relaxed for us. In any case, an interesting read for those of you that like
reading about consensus protocols.

Another interesting read that I would also recommend everyone interested in the project is our
[meeting notes from this week][meeting-notes]. They may be a bit intelligible, but there are a lot
of nice ideas that came out of our discussion and worth skimming through.

And to wrap this up, what's up for this week? As mentioned above, I want to have a first detailed
draft of the membership selection protocol in order to try to fit the segment verification it, and
see if it fulfills all our needs or if we need to explore other ideas.

As always, I'll keep you posted with the progress, and see you next week!

[apenglow-discussion]:
  https://abundance.zulipchat.com/#narrow/channel/495788-research/topic/Alpenglow.3A.20A.20New.20Consensus.20for.20Solana/with/520833964
[meeting-notes]:
  https://abundance.zulipchat.com/#narrow/channel/502084-meeting-notes/topic/2025-05-28/with/520884056
[dynamic-difficulty-discussion]:
  https://abundance.zulipchat.com/#narrow/channel/495788-research/topic/Dynamic.20difficulty.20adjustment.20and.20farmer.20allocation.20to.20shard/with/521003286
