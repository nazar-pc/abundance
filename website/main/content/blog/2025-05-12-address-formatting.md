---
title: Address formatting
date: 2025-05-12
draft: false
description: Implementation of address formatting and work on block structure
tags: [ status-update ]
authors: [ nazar-pc ]
---

The discussions with Alfonso and attempts to start building an actual blockchain led to spending a big part of last week
working on block structure. That work is not done yet. However, I like to have some sort of accomplishment at the end of
the week if possible, so I spent some time to finally implement the formatting of addresses, which will be the main part
of this update.

<!--more-->

## Block structure

Before getting to addresses, I'd like to share some challenges with the block structure. Block header and body look like
those things that were figured out a long time ago and do not need a lot of attention. However, in our sharded design,
we need to both include information from shards in their parent shards, and include reference to the beacon chain in
shards below. Not only that, this whole thing should be verifiable and allow to generate and inclusion of anything
recursively referenced by the beacon chain. Moreover, it must be possible to verify just from the beacon chain block
hash (or super segment root in case of archival history).

Those constraints require a lot of careful consideration about the order of things and how things are aggregated into
"block hash." Block hash is sometimes just a hash of the block header contents as bytes, but doesn't have to be exactly
that, strictly speaking.

So I've been working on a custom structure of the block header and block body that does allow to store all the necessary
information, while optimizing the size and verification complexity of generated proofs. A lot of interesting questions
popped up, even something as simple as [Whether to include timestamp in block header or not?] was up for debate.

[Whether to include timestamp in block header or not?]: https://abundance.zulipchat.com/#narrow/channel/495788-research/topic/Whether.20to.20include.20timestamp.20in.20the.20block.20header.20or.20not/with/517262114

I hope to finish the initial version of that by next update and explain the rationale behind key decisions.

## Address formatting

Now the key part of this update: what should the address in a blockchain look like?

I think we're all used to addresses looking like a long unreadable gibberish unless some system for aliases is used
like [ENS] (`.eth`) or [SNS] (`.sol`), neither of which is a part of the core protocol of Ethereum or Solana
accordingly. Sure, we need to encode some binary information that inevitably looks like gibberish, but there is more
than one way to skin a cat and not all of them are equally good.

[ENS]: https://ens.domains/

[SNS]: https://www.sns.id/

Let's first look at the way blockchain addresses look like for some popular blockchains and what are the good and
not-so-good things about them:

| Blockchain                   | Example address                                                                                                                            | Good                                                                                  | Bad                                                                                                                                                    | Ugly                                                           |
|------------------------------|--------------------------------------------------------------------------------------------------------------------------------------------|---------------------------------------------------------------------------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------|----------------------------------------------------------------|
| Ethereum (hex)               | 0x661cda03aba8d39&#8203;35f6b456f1668b987&#8203;03332333                                                                                   | Short-ish, simple unambiguous vocabulary                                              | Basically unreadable and unpronounceable, no indication of which chain it belongs to                                                                   | No integrity check, accidental typos can lead to loss of funds |
| Bitcoin ([Bech32]/[Bech32m]) | bc1qzqhl7npmadm56&#8203;nvmvm2pexmmkrc6m&#8203;msyjcdclrjzpzn6sn4vyn&#8203;usdzewtu                                                        | Bech32m supports strong error detection, avoids ambiguity, clear chain identification | Depending on address kind is even longer and less readable than Ethereum (like in provided example)                                                    |                                                                |
| Solana (Base58)              | 5ddo32xdfxBvxweFYe&#8203;SDbteK53Fj68fAVvVqy&#8203;RF6Mp&#8203;HY                                                                          | Unambiguous vocabulary                                                                | About as unreadable as Ethereum, slightly longer, no chain identification                                                                              | No integrity check, accidental typos can lead to loss of funds |
| Cardano (Bech32)             | addr1q9mu9r0ynn034&#8203;qwwm8lkncu9dd0864&#8203;cn8wd74rd5j90q3ydcq&#8203;228ammxm02j45gudl5&#8203;pgklvhvpkxx2stxth5pc&#8203;xv2xseh43q6 | Bech32 supports strong error detection, avoids ambiguity, clear chain identification  |                                                                                                                                                        | Extremely long, painful to type manually                       |
| Polkadot/Substrate ([SS58])  | 12pZ8VV6o3A6BFYg3&#8203;b3kk6TC1ZD53x9bXw&#8203;koFenv&#8203;183DEods                                                                      | SS58 supports error detection, avoids ambiguity                                       | Fairly long, chain identification is not as clear as in some other formats and causes some confusion due to the context where these addresses are used |                                                                |

[Bech32]: https://github.com/bitcoin/bips/blob/60ac0e8feccb07f891fd984e4ed76105d2898609/bip-0173.mediawiki

[Bech32m]: https://github.com/bitcoin/bips/blob/60ac0e8feccb07f891fd984e4ed76105d2898609/bip-0350.mediawiki

[SS58]: https://wiki.polkadot.network/learn/learn-account-advanced/

There are many more examples, but I think you get an idea. Neither option is truly great, so it is worth thinking what
it could be.

> Fun fact, I spent a non-negligible amount of time trying to format the above table so that it is at least somewhat
> readable, especially on mobile devices. Those addresses really do not want to fit on the screen, and there isn't any
> natural way to slice them up. This isn't a problem with the format described below.

If talking about balance transfers, there are examples from a world of fiat:

| Address type | Example address                            | Good                                                                                                     | Bad                                                                                                                    |
|--------------|--------------------------------------------|----------------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------------------------------------------|
| Card number  | 3782 8224 6310 8005                        | Reasonably short, only digits, services that respect users group digits into blocks of 4 for readability | No integrity check, though this is less catastrophic than in blockchain context, sometimes isn't formatted with spaces |
| IBAN         | CY17002001&#8203;2800000012&#8203;00527600 | Contains checksum for error detection                                                                    | Long gibberish that while can be, in most cases is not grouped in meaningful way for humans to read or type            |

One thing that impacts the length of the address is the amount of data stored in it. In Ethereum it is 20 bytes, in
Solana 32 bytes, Cardano clearly stores even more data there. As [mentioned previously], I decided to go with just 16
bytes in a form of `u128` for an address, which is an artifact of the smart contract design.

[mentioned previously]: ../2025-02-21-5-million-flips

Since Bech32m looks like one of the best options features-wise, let's use it as an example and format a random 128-bit
unsigned integer with `abc` prefix (human-readable part, Bitcoin mainnet uses `bc` for example):

```
abc1qzq2mr3rnsrwcka6fk6sdzcfv5ygv5w7
```

When compared to the blockchain alternatives above, it is already the shortest one, but it still looks very much like
gibberish that is unreadable and unpronounceable. We'll do something about that, but let's first talk about the contents
of that 128-bit unsigned integer that the address is.

The address logically has two parts:

* 20 bits at the beginning correspond to shard index (the least significant bits first)
* the remaining 108 bits correspond to an address allocated on that shard index (the most significant bits first)

While address can be used on any shard, it can only be allocated (when contract is deployed) on a shard that corresponds
to shard index.

Conveniently, since Bech32m is a base-32 representation, 20 bits of shard index correspond to exactly 4 characters in
the above example right after 1. With the last 6 characters being Bech32m checksum, we can break the address down into
components like this:

```
abc     1          qzq2         mr3rnsrwcka6fk6sdzcfv5 ygv5w7
^prefix ^separator ^shard index ^allocated address     ^checksum
```

Now let's re-assemble the address back with a few tricks:

* insert one separator after shard index
* insert another separator before checksum
* segment allocated address into groups of 4-3-4-4-3-4 characters

Now the address looks a bit more like a credit card number and should be easier to read and type:

```
abc1qzq2-mr3r-nsr-wcka-6fk6-sdz-cfv5-ygv5w7
```

Now it is a bit more readable, but it is still quite long, about as long as Ethereum address is. However, since the
shard index and allocated address are not random, we can make a few observations:

* when blockchain launches, there will not be many shards in existence, meaning out of 20 bits of shard index, most will
  be zero at the end: `shard 2: 01000 00000 00000 00000`
* similarly, addresses are allocated using a simple increment, so there will be a lot of zeroes at the start, for the
  fifth address allocated: `[105 zeroes]101`

So when we encode that as Bech32m, we'll get something that looks like this:

```
abc1gqqq-qqqq-qqq-qqqq-qqqq-qqq-qqq5-a7dw6e
```

See all of those `q`s in there? That is what `00000` bits encode to, and we have a lot of those. So the next logical
step is to strip all `q` to the right of the shard index and to the left of the allocated address, removing extra
separators in the process as well. Then we end up with this:

```
abc1g-5-a7dw6e
```

It is much shorter, can still be deterministically mapped back into full Bech32m and original `u128`, `abc1` is a fixed
prefix that is easy to recognize and remember.

This is exactly the format that I envisioned to use a few months ago, but only implemented last week in [PR 237].

[PR 237]: https://github.com/nazar-pc/abundance/pull/237

For a long time, the addresses will look not much longer than that and will only grow slowly as more contracts are
deployed on the blockchain. There are plenty of pain points related to blockchain adoption by regular users, and
addresses are a part of it. I hope this is a useful contribution not just to the [blockchain we're building], but to the
ecosystem more broadly.

[blockchain we're building]: ../2025-04-08-we-are-building-a-blockchain

## Upcoming plans

In the nearest future, I'm planning to finish the initial design of the block header and block body format and work
towards building consensus verification logic. There will probably be more discussions with Alfonso about how all of
these fits together, because there is an annoying number of moving parts that should fit together, hopefully in a
somewhat elegant way.

If you found any of this interesting and would like to discuss, please join our [Zulip]. In any case I should have more
updates to share next week.

[Zulip]: https://abundance.zulipchat.com/
