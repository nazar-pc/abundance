---
title: What is blockchain scalability?
date: 2025-07-13
draft: false
description: A rant about the confusion around "scalability" usage in blockchain space
tags: [ ]
authors: [ nazar-pc ]
---

There are buzzwords in any industry that are thrown around easily, and blockchains are no exception. In this post, I
want to focus on "scalability". Turns out when you say "blockchain scalability" different people hear different things.
The prevalent opinion seems to be that scalable blockchains are able to process more transactions than non-scalable ones
or something along those lines. Essentially making the ability to scale equivalent to peak performance.

Sure, peak performance is an important metric, although it is often a theoretical one. But I don't think that is the
most useful property, especially without clarifying the conditions under which it can be achieved.

<!--more-->

## "Scalable blockchain"

When looking around searching for scalable blockchains, [Solana] often appears as one of the most scalable options. What
that means in the context of the meaning at the beginning of this post is that it can handle a lot of transactions.

[Solana]: https://solana.com/

The catch? Solana validators have quite [high hardware requirements]. Processing more transactions is possible by
increasing those requirements further over time, which is [exactly the plan for Solana].

[high hardware requirements]: https://docs.anza.xyz/operations/requirements/

[exactly the plan for Solana]: https://solanacompass.com/learn/Lightspeed/solanas-ultimate-vision-anatoly-yakovenko#:~:text=How%20does%20Solana%20approach%20scalability%20differently%20from%20other%20blockchains

## "Infinitely scalable blockchain"

There are blockchains that go even further than that, claiming "infinite scalability", [NEAR does that] in articles and
on landing pages. NEAR's [hardware requirements for validators] are substantially lower though, so how is it possible
that a chain with lower hardware requirements for validators achieves [infinitely] higher scalability? Turns out, they
mean something completely different!

[NEAR does that]: https://medium.com/nearprotocol/how-nears-simple-nightshade-gives-dapps-infinite-scalability-433188d2ef1e

[hardware requirements for validators]: https://near-nodes.io/validator/hardware-validator

[infinitely]: https://factmyth.com/factoids/there-are-different-types-of-infinity/

What the team behind NEAR is actually trying to say is that while the requirements of individual validators are modest,
as more validators join the network, the network as a whole can process more transactions. And they seem to believe it
is possible to just keep adding infinite number of validators to achieve infinite transaction throughput.

## Who said rollups?

There is another quirky case I'd like to mention before giving my own definition. There are various flavors of
heterogeneous systems that are also called "scalable". For example, [Polkadot] has [parachains], [Ethereum] community
believes they are [scaling it through rollups].

[Polkadot]: https://polkadot.com/

[parachains]: https://wiki.polkadot.network/learn/learn-parachains/

[Ethereum]: https://ethereum.org/

[scaling it through rollups]: https://ethereum.org/roadmap/scaling/

In this case, neither the blockchain itself can process a particularly large number of transactions nor can it do so by
attracting more validators. What happens instead is that this blockchain (layer 1 or L1) is used as a settlement layer
for essentially other blockchains (layer 2 or L2), and since there could be multiple of those L2s, the whole
construction in its entirety can process more transactions.

## The definition

My definition is:
> Scalable blockchain is such that can process more transactions as more consensus participants (think physical
> machines) join the network

In fact, it is important to stress that most blockchains have **_inverse scalability_**!

100% of blockchains mentioned so far are based on Proof-of-Stake consensus, which has practical scalability issues
around the number of distinct consensus participants they could have. From registering on chain to network communication
complexity, it is basically guaranteed that a ceiling will be hit sooner or later despite all the excellent research
done with clever subcommittee sampling, validator pools and optimizations that avoid quadratic network complexity.

## Really scalable blockchain

I believe a really actually scalable blockchain must be able to scale the number of participants and increase its
throughput while doing so.

This is why this research project starts with a permissionless consensus design, which, just like [Bitcoin], can support
an enormous number of distinct consensus participants. Then the consensus is made hierarchical through sharding, such
that all those participants no longer execute the same exact thing over and over again (which wouldn't be scalable at
all).

[Bitcoin]: https://bitcoin.org/

Now a bit about "infinite scalability". Of course NEAR doesn't have it. They will not be able to add an infinite (or
even extremely large, like 1B) number of consensus participants, nor would their validators be able to physically
receive erasure coding pieces from an infinite number of other shards. So calling it "infinite scalability" is just a
marketing fluff.

In fact, it does help to have practical hard limits for various parts of the system. This allows to optimize and
simplify data structures, for example, by avoiding variable-length encodings.

In current sharding design, we have several of these hard limits. One is the solution range, which is represented by
`u64` and 64 bits is sufficient to represent all the disk space that currently exists in the world many times over and
for practical purposes is unlimited, but in principle it is not. If there is ever a sudden exponential advancement in
SSD capacity, additional sampling rules or other changes might be needed in consensus to scale the capacity further and
it wouldn't be that difficult.

Another hard limit is the number of shards. Shard index is an important parameter and its size has a wide range of
implications on things ranging from contract addresses to various inclusion proofs. We're using 20 bits for shard index,
which gives us ~1M shards in total, which feels like a really high number that the blockchain will likely never reach,
but it is a hard limit nonetheless.

Even if each shard processes a single user transaction, with 1M of them, it'll likely crush any public blockchain that
currently exists in terms of throughput already.

## Conclusion

I have never seen a comparable design to one that me and Alfonso are working on, that (under the above definition) is
actually scalable to Internet-scale performance.

Of course, there are many other important aspects of the blockchain beyond scalability. I believe there are good
solutions to many of them. Some of them are important enough on their own to justify the existence of a blockchain that
doesn't scale. However, true scalability remains one of those illusory goals that doesn't have a great solution yet. But
humanity deserves it!
