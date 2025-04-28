---
title: "Multi-shard Subspace Protocol"
date: 2025-03-31
draft: false
description: Desigining a partitioned version of the Subspace protocol for increased scalability.
tags: [ status-update, consensus ]
authors: [ adlrocha ]
---

After a lot of thinking, this week I came to the realisation that a sharded architecture like the one we are trying to
build can be designed leveraging the current design of the Subspace protocol and all its underlying mechanisms as a
base. While this was the idea from the beginning, either for lack of familiarity with the protocol or plain ignorance, I
was missing the big picture of how this could be done.

<!--more-->

Using Subspace as the core protocol for our design has several advantages:

- We can build upon the security and the robustness of the Subspace protocol. If we can assume the Subspace protocol as
  secure (which currently is the case), we can derive the security of our designs from the security analysis of
  Subspace.
- The team is already familiar with Subspace, this will help with its implementation and with reasoning about its
  correctness.
- We are building upon a consensus algorithm that is operating in production, and we can learn from all the improvements
  and mistakes made throughout its life (design and implementation-wise). Funnily, on our brainstorm sessions Nazar has
  already shared
  a [few](https://forum.autonomys.xyz/t/farmer-equivocation-is-problematic/3063?u=nazar-pc) [pointers](https://forum.autonomys.xyz/t/change-derivation-of-pospace-seed-and-hdd-compatible-exploit/4438?u=nazar-pc)
  to ideas that they already explored in the past and that are still relevant to us (which really helps when evaluating
  ideas for the design).

## High-level intuition for a multi-shard Subspace protocol

At this point you may be wondering already how does a multi-shard Subspace protocol would look like. I still don't have
the low-level details for the design, but at least I already have the really high-level intuition of how this could
work.

- As discussed in previous documents, ideally the protocol should be recursive and allow for infinite horizontal scaling
  by including deeper layers of shards
- The beacon chain (or main chain as referred to in some of the literature), is the main chain responsible for
  orchestrating the lifecycle and securing all the shards of the system. As such, all the population of farmers in the
  system participate from the system (see image below).
- By being part of the beacon chain, farmers are also implicitly proposing blocks from the underlying shards.
  High-level, the idea for the design is that the history buffer is populated with records belonging to the history of
  all shards in the system, and each of proof-of-time slot a new winning ticket will be drawn for each shard. The farmer
  encountering this winning ticket is responsible for sealing and broadcasting the newly proposed block for the shard
  they have the winning ticket for.

<p align="center">
<img alt="Distribution of farmers for multi-shard Subspace protocol" src="2025-03-31-farmer-distribution.png">
</p>

- Shards can be created in the beacon chain's genesis, or later in the history of the chain (we will leave the specifics
  about this process to the future. New shards will be spin off through
  a [network velvet forks](https://www.nmkr.io/glossary/velvet-fork-in-blockchain) [[1]]())
- The core idea for the system's consensus algorithm is that consensus participants will be contributing to a global
  history buffer, that will be consequently archived and used for the block winner election in each shard.
- The beacon chain does not allow user transactions, it only accepts system transactions and block commitments from
  shards in the immediate layer below.
- Each shard has its own independent transaction pool with transactions initiated in that chain.
- Each Proof-of-Time (PoT) chain slot, the randomness beacon triggers a global farming process, where each farmer runs a
  protocol to extract a winning chunk. Ideally, there should be at least a winner for each shard. The way in which we
  determine if the winning ticket belongs to a specific shard is by checking to which shard the winning chunk belongs.
- When a farmer creates a block for a shard it broadcast the block to the corresponding shard and the header to the
  beacon chain. In this way, blocks in the beacon chain aggregate all the lower layer blocks that have been proposed so
  far. Blocks in the beacon chain are included in the history buffer, and as this blocks include the headers for the
  underlying shard blocks, the history buffer implicitly includes all shards blocks.
- The archiving is done over this global history, and when a new shard block is encountered in a genesis block, its
  content needs to be pulled to included in the archived history.
- Not all farmers need to keep the state of all shards (as it would deem the use of a sharded architecture useless). As
  such, every epoch (determined by a window of `N` slots) there is a random assignments for farmers to shards as "
  executors". This assignment prevents potential collusion among farmers by keeping shard membership static. This epochs
  should be large enough to compensate for the "warm up" period between epoch changes where farmers may need to pull the
  latest state for their new shard if they don't have it.

<p align="center">
<img alt="High-level diagram of merged farming idea" src="2025-03-31-merged-farming-idea.png">
</p>

## From high-level ideas to low-level design

The high-level description shared above is great to gain an intuition of how a sharded version of the Subspace protocol
could look like. Unfortunately, after a few brainstorm sessions with Nazar, we started to find holes and added
complexity in how I imagined the protocol to work.

- How can we ensure that the history buffer is populated with records belonging to the history of all shards in the
  system?
- What farmers are responsible for contributing blocks (or segments) from a shard to the history buffer if not all
  farmers have access to the full state of all of the shards?
- How are farmers assigned to specific shards and how can we prevent collusion among farmers?
- Which farmers are allowed to propose blocks in a specific shard? Every farmer independently of the shard? Only farmers
  assigned to that shard?
- How can we align incentives to prevent selfish farmers from just doing the least possible work to get the rewards?
- How can we balance the population of farmers among shards to avoid a big farmer trying to attack a shard and lead to
  power dilution?
- How should we recover from an attack in a shard?

So you see that there are a lot of unanswered questions. With all of this in mind, we narrowed a bit more the design
space coming up with the following ideas --exploring these will be my focus on the coming week--:

- Instead of having a global archiving protocol that requires every farmer to have knowledge about the state in every
  shard, there will be independent archiving in each shard.
- Shards will notify new segments to their parent chains and the beacon chain by submitting segment headers. New
  segments in shards are created with a local sequence ID. When the segments are committed to the beacon chain, they are
  assigned a global ID that sequences shard segments into a global history buffer (see figure below with a rough
  illustration of how this could work).
- Farmers are notified about new segments. If there are new pieces within their cache limit they try to go to the DHT
  and fetch a piece to include in their sector.
- Plots are tight to a specific shard. Farmers commit to farm in full branches of the hierarchical tree where they will
  be entitled to farm new blocks. In order to be able to do so, they'll need to dedicate their storage to plotting on
  those shards (as it happens in single-chain Subspace). Along with keeping shard plots for all the shards in that
  branch of the tree, they will obviously also need to track the transaction pool for unverified transactions, and keep
  at least the most recent state for the shard.
    - With this approach, farmers can self-assign themselves to shards, but they are required to perform some upfront
      work to be able to farm a block (preventing them from jumping from one shard to another with low effort).
- Even if they are committed to a specific branch of the tree, farmers can do light verification for other shards in the
  tree like validating the headers of the rest of the shards.
- We should introduce mechanisms, like a power threshold, used to identify when a shard is in a weak state to rebalance
  the population of farmers among shards. We can probably get some inspiration for this
  from [Eigenlayer](https://eigenlayer.xyz/), where farmers can use their power to intervene in a shard and propose
  blocks to fix it. This same process should also be used to identify and recover from an attack in a shard.
- Finally, on top of all these mechanisms we can come up with a reward system that forces rational balancing of the
  farming population among shards. This will help us avoid collusion and a big farmer trying to attack a shard.

<p align="center">
<img alt="Nazar's high-level diagram of refined merged farming" src="2025-03-31-nazar-diagram.png">
</p>

## Core subprotocols for the design

And I couldn't close this weekly update without sharing a really interesting paper that Nazar brought to my attention
throughout the week, and that ended up being extremely relevant to what we are
doing: [Scalable Multi-Chain Coordination via the Hierarchical Longest Chain Rule](https://ieeexplore.ieee.org/document/9881846).
This paper introduces BlockReduce, a PoW-based blockchain that achieves high-throughput by operating a hierarchy of
merged mined parallel chains.


<p align="center">
<img alt="Blockreduce hierarchical architecture" src="2025-03-31-blockreduce-image.png">
</p>

The paper presents a hierarchy of Nakamoto-based consensus chains like we have, and it introduces a lot of interesting
concepts that reinforces or adds up to all of the ideas that we've been having in the past few weeks.

- They introduce the concept of merge mining, where miners simultaneously mine multiple chains. This is similar to what
  we are trying to achieve with the Subspace protocol, where farmers will be able to farm multiple shards at the same
  time by choosing a branch of shards in the hierarchy.
- They propose the concept of coincident blocks, which are blocks that share the same PoW solution. This is a really
  interesting concept that we can use to have an implicit order of the different partitions (shards) through coincident
  blocks.
- They also propose the concept of cross-net transactions that need to be validated and can only be executed when the
  transactions are included in a coincident block.
- They use a burn-and-mint operation for cross-net transactions which really aligns with our idea of limiting cross-net
  transaction to the basic atomic operations. Even more, these transactions can only be executed when coincident blocks
  happen, as they are the ones that can attestate ordering among different shards.
- Finally, BlockReduce leverages a rational model, where each miner self-assigns itself to the hierarchical path that
  they consider more profitable for them (which is the model we are leaning towards after our latest brainstorms).

## What's next?

We are starting to having a sense of how we can implement a hierarchical version of the Subspace protocol. The next step
is to start breaking down the design into smaller pieces and start reasoning about their security and correctness. We
need to identify what are the core subprotocols that would be needed to implement a hierarchical version of the Subspace
protocol, which ones need to be adapted, and which ones can be reused unchanged.

This week my focus will be on trying to flesh out a first detailed spec for what I am calling the merged farming
protocol, i.e. the construction of a global history buffer, a sharded archiving and plotting protocol, and sharded
farming for shard block generation. On top of this, I'll also explore alternatives for the clustering protocol, i.e. how
are farmers assigned to specific shards (or partitions).

So with nothing else to add, see you next week!
