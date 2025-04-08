---
title: We are building a blockchain
date: 2025-04-07
draft: false
description: About what are we doing and why
tags: [ announcement ]
authors: [ nazar-pc ]
---

[Welcome] post mentioned briefly [initial set of constraints] that led to the creation of this project, but I figured it
might be helpful to have a short writeup about it that might be helpful for sharing.

In short: we're building a blockchain.

By "we" I really mean just me and Alfonso so far, but I hope more people will join over time if they find it
interesting.

[Welcome]: ../2025-01-13-welcome

[initial set of constraints]: https://gist.github.com/nazar-pc/760505c5ad7d56c20b2c75c1484e672f

<!--more-->

## A blockchain

Yes, we are simply building a blockchain. Just a blockchain, not a blockchain for AI, finance or data storage.

All of those things can be built on a blockchain that actually scales, but no one can predict how new technology will be
used eventually, so it is important to distinguish: what the thing is from what it can be used for.

## Why?

Blockchains started as a technology supposed to allow everyone to participate in a distributed permissionless P2P
network, where and arrive at consensus for a set of state transitions without trusting anyone.

Unfortunately, blockchains today are neither permissionless nor distributed, with a lot of trust assumptions and not
scalable at all. Frustratingly, I don't see anyone actually trying to fix all of it. Some are fixing some parts of the
issue, but a comprehensive solution is lacking.

Proof-of-work that wastes computation went out of favor some time ago, partly because of energy consumption and partly
because it ended up fairly centralized in practice. Proof-of-stake dominates the landscape these days but is no longer
permissionless and not decentralized either. As of today, two biggest Bitcoin mining pools (Foundry USA and AntPool)
have more than 50% of the hashrate, while top-5 produce more than 77%. The situation with Ethereum is not better with
top-2 (Lido pool and Coinbase), resulting in more than 50% of the stake with top-5 having more than 84% of the total
staked ETH. This is not the future I'm looking forward to.

The problem is not just that it is not decentralized, those protocols inherently can't be decentralized. Any
proof-of-work protocol that gets popular ends up centralized pools, any proof-of-stake ends up with centralized stake. I
gave examples of two biggest networks above, but the same exact thing happens across the board.

Obscure languages and execution environments dominate the landscape with Solidity/EVM arguably being the biggest one.
Why do we have to reinvent compilers, rewrite cryptographic libraries over and over again? There is so much software
written already, wouldn't it be better to be able to include a normal generic C library in your smart contract if it
gets the job done?

The last big issue I have with most blockchains is that they either aren't even trying to scale or claim to be scalable
without actually being scalable. My definition of "scalable" is that the network is able to store and process more
transactions as more participants join the network, without an upper bound. Ethereum's microcontroller-like compute
capabilities are not enough, Solana's vertical scalability can't possibly satisfy all possible demand.

## So what?

You might agree with everything above, but wondering "so what?"

I believe that due to countless shortcomings, many interesting applications are simply not being built, not even
attempted to be built. A lot of real issues that could be solved with blockchain technology aren't solved because of it.

## The solution

What we're building is a blockchain to solve all the above issues and then some.

We're building a blockchain that can support literally any number of consensus participants. With
Proof-of-Archival-Storage consensus, individual participants pledge disk space to store the history of the blockchain.
The goal is to have a weekly payout for each terabyte of space pledged even as the blockchain gets enormously large,
making pools pointless and real decentralization possible.

We're building a blockchain that scales through sharding. The practical constant for max number of supported shards is
expected to be one million, with no inherent limit at the protocol level. This means the ability to upload data to the
blockchain at a rate of terabits per second and beyond. This also means the ability to get real compute done on chain
with millions of modern high-performance CPU cores.

We're building a blockchain that allows running applications written in traditional languages, with Rust being the
primary target. It will be possible to debug and optimize using traditional tools like gdb and perf. With RISC-V ISA and
support for standardized extensions, the code is fast and has access to modern hardware accelerators for things like
cryptography without using obscure custom opcodes and VM-specific code, use high-quality high-performance libraries that
already exist.

We're building a blockchain that is future-proof with post-quantum cryptography.

We're building a blockchain that can describe itself through metadata embedded into smart contracts, so you'll never
have to do blind signing or trust the computer when using a hardware wallet.

In the end, we'll have a user-friendly blockchain that supports billions of transactions per second without compromising
on security or distributed nature of the protocol.

## What is happening?

We're building a blockchain already, but would love to collaborate and discuss ideas with others.

Join our [Zulip] chat for discussions and check [Contribute] page for the most pressing issues that are not being worked
on right now, but should be.

[Zulip]: https://abundance.zulipchat.com/

[Contribute]: /book/Contribute.html
