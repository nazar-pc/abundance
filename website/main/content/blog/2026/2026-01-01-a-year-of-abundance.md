---
title: A year of Abundance
date: 2026-01-01
draft: false
description: A brief recap of what has happened to the project in the past year
tags: [ announcement, status-update ]
authors: [ nazar-pc ]
---

It has been around a year since the project started, and I would like to share a brief summary of what has happened so
far, where the project is at and what's coming next. I'll only capture the highlights and key outcomes here, presented
in rough chronological order.

<!--more-->

## December 2024

In December 2024 I have officially transitioned out of [Autonomys Labs] (previously known as Subspace Labs) and started
working on "Project Abundance" as an independent R&D project with the sponsorship from [Subspace Foundation] (for which
I'm very grateful).

[Autonomys Labs]: https://www.autonomys.xyz/

[Subspace Foundation]: https://subspace.foundation/

The first commit in the repository was made on December 27, 2024.

## January 2025

I had a conviction that the way smart contracts development is done in existing blockchain ecosystems is suboptimal and
is not designed for performance. So my first goal was to think from the first principles what a good, high-performance
design could look like.

I researched existing blockchains and did a bunch of prototyping and experimentation. By the end of the month, contracts
were [almost running] with a lot of initial infrastructure and some early documentation available. There was no
execution environment available yet, there was definitely no blockchain, but it was sufficient to get an idea of what it
would look like to write contracts for a future system, what benefits, limitations and challenges it would have.

[almost running]: ../2025-01-30-contracts-are-almost-running

## February 2025

At the beginning of February I finally [implemented the first version] of the native execution environment, which
finally allowed running basic contracts with some basic documentation for the `#[contract]` macro.

[implemented the first version]: ../2025-02-07-contracts-are-actually-running

Same month I was [talking to some developers] to collect initial feedback about the contract design, which helped with
the APIs, documentation improvements and generally was a great opportunity to get some external feedback.

[talking to some developers]: ../2025-02-14-initial-developer-feedback

I also spent some time tuning the performance of the core data structures related to execution of contracts to get an
idea of the low-level overhead required before any contract logic even begins executing, which turned out to be tiny,
especially for the native execution environment.

## March 2025

In March, with contracts being somewhat usable, I went to the next layer in the stack: [transactions]. I spent a lot of
time thinking about how transactions should work in a sharded blockchain and came up with a generic design, where the
verification of transactions is done by a "wallet" contract. This way, there is a huge design space for implementation
of contracts, with the blockchain itself not being aware of the signature verification logic or similar concepts,
usually hardcoded into the core protocol of other blockchains.

[transactions]: ../2025-03-02-transactions

This was also the month when Subspace Foundation sponsored [Alfonso] to help me with research of the sharding design.

[Alfonso]: https://www.adlrocha.com/

By the end of the month there was a way to build contract files in ELF format. We also had productive discussions about
the way [hierarchical sharding] might work with Proof-of-Archival-Storage consensus.

[hierarchical sharding]: ../2025-03-31-hierarchical-subspace-protocol

## April 2025

April [continued with the sharding design research] and [preparations] towards having an actual [blockchain] one day.
Eventually I started seeing [trees everywhere], and I must admit, I continue seeing them everywhere to this day.

[continued with the sharding design research]: ../2025-04-07-merged-farming

[preparations]: ../2025-04-07-preparing-for-blockchain

[blockchain]: ../2025-04-08-we-are-building-a-blockchain

[trees everywhere]: ../2025-04-14-trees-everywhere

By the end of the month I have [refactored] and massively [improved the performance] of some components forked from
Subspace.

[improved the performance]: ../2025-04-20-very-fast-archiving

[refactored]: ../2025-04-28-subspace-codebase-refactoring

## May 2025

In May, I [continued tinkering] with forked Subspace components with the goal of shaping them closer to what I wanted
and not depending on [Substrate] since I was not planning to use it.

[continued tinkering]: ../2025-05-05-subspace-codebase-refactoring-part-2

[Substrate]: https://github.com/paritytech/polkadot-sdk/tree/master/substrate

A big and nice accomplishment of this month was the user-readable [formatting for contract addresses]. It was a lot of
research and is inherently linked to the sharding design, I'm quite happy with the way it turned out.

[formatting for contract addresses]: ../2025-05-12-address-formatting

By the end of the month, Alfonso also [started seeing trees] everywhere in the design. We were also working on
[a way to organize the hierarchical shards] in a way that allows having a reasonable confirmation of blocks and
archived history, such that we can actually have efficient cross-shard communication.

[started seeing trees]: ../2025-05-19-block-as-forest

[a way to organize the hierarchical shards]: ../2025-05-26-longest-chain-rule-and-blocks-probability-of-reorganisation

As a result of a lot of brainstorming and all the discussions, I finally designed a shard-native [block structure] for
the beacon chain, intermediate shards and leaf shards, such that they form a hierarchical 3-level tree. It was a long
and tedious process, but crucial to any progress down the line.

[block structure]: ../2025-05-19-what-does-a-block-look-like

## June 2025

In June, we discussed [how to assign] [farmers to shards], although the decision and especially implementation
materialized much later.

[how to assign]: ../2025-06-02-membership-selection-and-segment-verification

[farmers to shards]: ../2025-06-09-modelling-membereship-allocation

I [started] [making] [steps] towards having initial block production and stumbled upon many opportunities for
performance
optimizations, which was quite rewarding to explore and continues to be the trend to this day. Modern hardware can do
***so much*** when software is designed with performance in mind!

[started]: ../2025-06-02-path-to-block-production-and-procrastination

[making]: ../2025-06-16-beacon-chain-block-processing-infrastructure

[steps]: ../2025-06-23-block-processing-progress

We explored the topic of [sector expiration] extensively, but it has proven to be hard to come up with something robust
that is also reasonably simple to understand and implement.

[sector expiration]: ../2025-06-30-expiring-sharded-subspace-plots-and-improving-model-script

## July 2025

July marked [the start] of a lengthy process of tinkering with an awesome [rust-gpu] after exploring a few alternatives
that ended up being much less awesome. Performance improvements continued to pour in from various fronts, making
everything much faster still.

[the start]: ../2025-07-02-adventures-with-rust-gpu

[rust-gpu]: https://rust-gpu.github.io/

Exploration of sector expiration and a way to commit farmers to shards continued, though nothing better than
[the previous design] was ultimately found.

[the previous design]: https://abundance.zulipchat.com/#narrow/channel/495788-research/topic/A.20radically.20simple.20farmer.20allocation.2Fsector.20expiration/with/527356280

I continued building foundational components like [Sparse Merkle Tree] and client database.

[Sparse Merkle Tree]: ../2025-07-20-sparse-merkle-tree-and-client-database-preparation

Unfortunately, this was the month when Alfonso's contract was terminated by Subspace Foundation, so I was the only one
working from this month onwards.

## August 2025

In August, I finally managed to put together a very custom [client database prototype] and worked
on [transaction processing design]. I even created [a node prototype], finally, even though it wasn't capable of doing
much just yet.

[client database prototype]: ../2025-08-01-client-database-prototype

[transaction processing design]: ../2025-08-08-async-transaction-processing

[a node prototype]: ../2025-08-17-node-prototype

As I was tinkering with rust-gpu all this time and working on implementing sector encoding on the GPU, I
[kept discovering ways] to make it faster on the CPU too. In the end, CPU plotting is more than 2x faster than Subspace
repo at the time and even today.

[kept discovering ways]: ../2025-08-26-faster-proof-of-space-part-1

## September-October 2025

I think it is safe to say that I spent most of September and October on GPU programming. I learned [a lot] [more] [ways]
still to accelerate Proof-of-Space both on CPU and GPU. Ultimately, I was able to [get GPU plotting to work].

[a lot]: ../2025-09-07-faster-proof-of-space-part-2

[more]: ../2025-09-18-faster-proof-of-space-part-3

[ways]: ../2025-10-14-faster-proof-of-space-part-4

[get GPU plotting to work]: ../2025-10-23-gpu-plotting-works

## November 2025

Then there was almost a month of work gap as I was trying to get GPU plotting to work at a reasonable speed since the
initial version wasn't very fast. I didn't post any updates during this time as it was probably not as interesting to
anyone following.

What was interesting is that by the end of November I reached a key milestone, the node prototype mentioned
before [started producing beacon chain blocks]!

[started producing beacon chain blocks]: ../2025-11-24-the-first-block

Unfortunately, this was also the month when Subspace Foundation sponsorship ended for me too, so at this point I'm alone
and living off my savings.

## December 2025

The first produced block achieved in November was nice. However, there were plenty of errors shortly after that, so my
December started with fixing a bunch of things to the point that I was able to [produce 10k blocks], which involved more
consensus processes like solution range adjustment, Proof-of-Time entropy injection and block archiving.

[produce 10k blocks]: ../2025-12-11-steps-towards-multi-shard-farming

I then started taking [steps towards multi-shard farming] and implemented a [permissionless shard assignment] designed
back in June.

[steps towards multi-shard farming]: ../2025-12-11-steps-towards-multi-shard-farming

[permissionless shard assignment]: ../2025-12-19-permissionless-assignments-of-farmers-to-shards

By the end of the month I decided to work on something more rewarding again related to contracts. After some research
and experimentation I landed the [initial version] of RISC-V interpreter, CLI for building contracts and defined a
contract file format no longer based on ELF, but one that can be converted from (and eventually to) ELF.

[initial version]: ../2025-12-29-contracts-cli-and-risc-v-interpreter

## Today

And today is January 1st 2026, so I can't tell you how this year ends until we're through. But I am sure there will be
key developments with contract execution, I expect block production of intermediate shards and leaf shards at some point
later this year, and maybe we'll even get some networking between the nodes to sync from each other in a small devnet.

Definitely a lot of exciting stuff ahead, can't wait to see where we end up one more year from today.

## Acknowledgements

I'd like to thank again the Subspace Foundation for their financial support, Alfonso for helping with research, Shamil
and Liu-Cheng for fruitful developer feedback and everyone else I had a chance to discuss things with in some capacity.

I also want to thank [Zulip] for providing free access to "Zulip Cloud Standard" and [Graphite] for free access to their
AI code review service.

[Zulip]: https://zulip.com/

[Graphite]: https://graphite.com/

And certainly to Subspace Labs (now Autonomys Labs) for original research and implementation of the Subspace protocol,
none of this would have been possible otherwise, especially to [Jeremiah] and [Dariia].

[Jeremiah]: https://www.linkedin.com/in/jeremiah-wagstaff-483b5057/

[Dariia]: https://x.com/FutureDies

If you want to find me, [Zulip Chat] is the best way to start a conversation on topics related to "Project Abundance."

[Zulip Chat]: https://abundance.zulipchat.com/
