---
title: Drawing inspiration from the Internet's architecture to scale consensus
date: 2025-03-16
draft: false
description: Thinking about the foundational architecture to achieve Internet scale.
tags: [ status-update, consensus ]
authors: [ adlrocha ]
---

I want to kick-off my first weekly update in the project thanking Nazar for the warm welcome and the opportunity to work with him on this exciting project. I was really pumped to see other teams actively working on a similar problem to the one I started researching more than three years ago. For several reasons, I wasn't actively contributing to this problem any more, but this opportunity was the perfect excuse to get back to the game.

> If you are curious about my previous work on the matter before joining Nazar, feel free to skim through [this paper](https://ieeexplore.ieee.org/abstract/document/9951359) to get the gist of it. 

## Background
First things first, what exactly is this problem that I am referring to? At least the description of the problem is simple, _"we want to design blockchain infrastructure that can scale to the size of the Internet"_. The system should be able to host applications ranging from high-throughput media-intensive social networks and virtual worlds; to those that require more strict trust requirements and security guarantees, likeweb3-native and financial applications.
Unfortunately, the implementation of a system like this is very challenging. Current blockchain designs are still extremely monolithic and require the execution of transactions to be performed by (almost) every node in the system, and for the state to also be replicated in each (or a great number) of them. All the innovations around L2s, rollups, and some next-gen blockchains are improving this, but no one is close to achieving a system that is able to operate at the scale of the Internet in a seamless (and if I may add, UX-friendly) way.

## The architecture of the Internet
The best way to design a distributed system that is Internet-scale and that supports the workloads currently being executed in it is to build it from first principles, go to the source, and derive the architecture of our candidate system directly from the Internet.

If we look at how the Internet is structured today, we see the following layered architecture:

* A network of data centers holding the global state for all applications and the computational resources required to run applications.
The interconnection of different Autonomous Systems that enables the exchange of information between different subnetworks through a routing system, logically merging all of the state and resources in the system.
* Local networks with a large number of devices that hang from an AS, and depend on it to interact with the rest of the network.
* A hierarchical DNS System that provides naming resolution through a distributed hierarchy, offering lessons for decentralised naming and discovery.

How can we build a consensus algorithm that resembles this hierarchical architecture of the Internet, where there are different subnetworks that are globally orchestrated to operate as a common system? This has been the main focus for me this week, to think about how the high-level architecture of this consensus algorithm would look like considering the "wish list" set by Nazar [here](https://gist.github.com/nazar-pc/760505c5ad7d56c20b2c75c1484e672f).

## A layered consensus
The first thing that we should acknowledge for our candidate designs is that there is a single "one-size-fits-all" consensus algorithm able to support any kind of application, but we want to come up with a design that offers the basic infrastructure that can be configured to make this possible. Depending on the application, the throughput and security guarantees are different. However, using a sound core consensus that can be tweaked and integrated as part of a bigger global consensus with the right primitives may be able to work around some of these trade-offs and achieve the desired result.

With this in mind, this is the high-level architecture that I've been tinkering with to guide my design:

### Layer 3: Local Area Consensus
- Application-specific consensus run in local area subnetworks (LANs).
- They form dynamic clusters based on network proximity and transaction patterns
- Ideally, they run a consensus algorithm that allow for sub-second finality for local transactions. This consensus algorithm should be light and allow applications to configure its parameters for their needs.
- Provide strong local consistency with BFT guarantees `(3f+1)`
- Operate independently during network partitions
- Membership should be Subscription-based. They use a topic-based-like membership, where nodes that want to participate in a local are subnetwork can subscribe and unsubscribe dynamically (the subnetwork will be active as long as there are members).
- Proof-of-Archival should still be applied in local area networks to ensure that the history of the LAN is stored, and so we can leverage PoS for sybil resistance.
- The way in which I am considering this to be implemented is that LANs create fast microblocks that are broadcast to members subscribed to the LAN and the WAN (or WANs) that the LAN hangs from.

### Layer 2: Wide Area Consensus
- Aggregates local clusters into wide-area regions
- Aggregate microblocks from underlying LANs and checkpoint their state through macroblocks (combining microblocks and WAN transactions).
- Handles cross-cluster transactions within a region
- Runs a Nakamoto-based consensus based on Subspace's consensus basic primitives (i.e. employs erasure coding for data availability and Proof-of-Archival).
- All WANs are equal in terms of capabilities, and farmers are randomly assigned to more than one WAN (depending on the farmer population). 
- The target block times of WANs should be similar (or better) to the ones that Autonomys currently have.
- WAN (as is the case for LANs) have full blockchain functionalities (transactions, smart contract executions, etc.).

### Layer 1: Global Consensus
- Offers the higher level of security and data availability.
- The whole population of farmers in the system have to participate from the global consensus, as is the one orchestrating all the WANs (and implicitly LANs).
- There is no support for smart contract execution in the global network, and it serves exclusively as an anchor of trust, and a system-wide consensus network (with basic system functionality, like a global name system, account management, WAN farmer membership, synchrony, etc.). The global consensus run a global network analogous to the role that the beacon chain currently has in Ethereum.
- Provides probabilistic finality that strengthens over time.

## What's next?
Obviously, many projects, from Polkadot to Optimism or Cosmos, have already realised that the best wayy to scale blockchains to support different kinds of applications is to deploy a set of subnetworks that are able to operate as a whole. So the architecture I described above doesn't add much innovation in itself. However, I think that the key to achieve global scale is on the underlying mechanics. Using the system should be as seamless as using the Internet today.

For the first few strokes I will focus on designing Layers 1 and 2. This next week I want to focus on coming up with the membership protocol responsible for assigning, and the lifecycle of a transaction in the hierarchy i.e. how blocks are created, validated, and executed in the different layers, and the mechanism used to store and interleave the history of all the networks in the system. 

To help me with this I am going to review again these papers for inspiration:
- [Bicomp: A Bilayer Scalable Nakamoto Consensus Protocol](https://arxiv.org/abs/1809.01593)
- [Close Latency-Security Trade-off for the Nakamoto Consensus](https://arxiv.org/abs/2011.14051)
- [Nakamoto consensus with VDFs](https://ar5iv.labs.arxiv.org/html/1908.06394)
- [Phantom GHOSTDAG](https://eprint.iacr.org/2018/104.pdf)
- [Proof-of-Stake Sidechains](https://eprint.iacr.org/2018/1239.pdf) (I really loved this one the first time I read it!).
- [Narwhal and Tusk: A DAG-based mempool and efficient BFT Consensus](https://arxiv.org/pdf/2105.11827)

And that concludes my first project update. I'll try to get you a few juicy updates next week. Until then!