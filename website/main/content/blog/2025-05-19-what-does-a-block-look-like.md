---
title: What does a block look like?
date: 2025-05-19
draft: false
description: Initial version of the block header and body layout for different kinds of shards
tags: [ status-update ]
authors: [ nazar-pc ]
---

The question might seem somewhat obvious: you have a header and a body with transactions, many blockchains have it, what
might be so difficult about it? Well, as I [mentioned in the previous update], there are some complications and part of
the challenge is related to the fact that we're dealing with sharded architecture, which most blockchains don't need to
deal with.

[mentioned in the previous update]: ../2025-05-12-address-formatting/#block-structure

<!--more-->

## Sharding architecture

The sharding design is not fully worked out yet, but it is quite certain that it'll be hierarchical and look something
like this:

```goat
                         Beacon chain
                         /          \
     Intermediate shard 1            Intermediate shard 2
             /  \                            /  \
Leaf shard 11   Leaf shard 12   Leaf shard 22   Leaf shard 22
```

So it is a tree with a **Beacon chain** at the top of the hierarchy, with **intermediate shards** below it and **leaf
shards** at the bottom. The architecture supports about one million shards in total.

The shards are using a shared namespace for addresses and support cross-shard communication with both the ability to
send information between shards and using contracts deployed on any shard on any other shard.

All this implies the need to be able to prove and verify that things exist or have happened on other shards, while only
having access to the beacon chain block root. While at it, we'd like for proofs to be efficient too, which means both
block header and block body need to be designed in a way that co-locates related information and minimizes the depth of
various Merkle Trees.

In addition to cross-chain communication, we have to combine segments produced by individual shards into a global
history, which also needs to be represented in a block in some verifiable way.

While the actual number of shards will be larger than on the above diagram, the number of layers is expected to be the
same. This also means that both headers and bodies for different kinds of shards will be different. For example, the
beacon chain will have no user transactions and leaf shards will have no shards below it. There are many more
differences, of course.

## Current state

While the current version is certainly not final, I think it'll give a good idea of what to expect and how things work
together. I'll visualize it as a tree of trees, where you can think of every tree as a Merle Tree. It is about 95%
accurate (there are segment root proofs missing on the diagram due to the difficulty to show them correctly).

### Leaf shard block

Leaf shard blocks are the simplest. They reference the beacon chain in the header (which defines some consensus
parameters) and include segment roots and transactions in the body.

```goat
                                                                 -+
                            +---- Prefix -+- version              |
                            |             +- number               |
                            |             +- shard index          |
                            |             +- timestamp            |
                            |             +- parent root          |
                            |             +- mmr root             |
                           /                                      |
                          +------ Result -+- body root            |
                         /                +- state root            \
                        /                                           + Generic
                       +- Consensus info -+- slot number           /  header
                      /                   +- proof of time        |
                     /                    +- future proof of time |
         +- Header -+                     +- solution             |
        /            \                                            |
Block -+              +------------ Seal -+- public key           |
        \              \                  +- signature            |
         \              \                                        -+
          \              \
           \              +--- Beacon chain info -+- block number
            \                                     +- block root
             \
              \
               +- Body -+--- Own segment roots -+ 
                         \                       \
                          +- Transactions -+      +- [ segment root ]
                                            \
                                             +- [ transaction ]
```

### Intermediate shard block

Intermediate shards on top of what is done by leaf shards need to include information about leaf shards in both block
header and body.

```goat
                                                                 -+
                            +---- Prefix -+- version              |
                            |             +- number               |
                            |             +- shard index          |
                            |             +- timestamp            |
                            |             +- parent root          |
                            |             +- mmr root             |
                           /                                      |
                          +------ Result -+- body root            |
                         /                +- state root            \
                        /                                           + Generic
                       +- Consensus info -+- slot number           /  header
                      /                   +- proof of time        |
                     /                    +- future proof of time |
         +- Header -+                     +- solution             |
        /            \                                            |
Block -+              +------------ Seal -+- public key           |
        \              \                  +- signature            |
         \              \                                        -+
          |              \
          |               +--- Beacon chain info -+- block number
          |                \                      +- block root
          |                 \
          |                  +- Child shard blocks -+ 
          |                                          \
          |                                           +- [ block root ]
          +- Body -+--- Own segment roots --+
                    \                        \
                     \                        +- [ segment root ]
                      \
                       +-------- Leaf shard block headers -+
                        \                                   \
                         \                                   +- [ block header ]
                          +- Leaf shard segment roots -+
                           \                            \
                            +- Transactions -+           +- [ own segment roots ]
                                              \
                                               +- [ transaction ]
```

### Beacon chain block

Beacon chain replaces reference to the beacon chain with actual consensus parameters. It still tracks child shards, but
also contains PoT checkpoints in its body for faster verification later.

```goat
                                                                 -+
                            +---- Prefix -+- version              |
                            |             +- number               |
                            |             +- shard index          |
                            |             +- timestamp            |
                            |             +- parent root          |
                            |             +- mmr root             |
                           /                                      |
                          +------ Result -+- body root            |
                         /                +- state root            \
                        /                                           + Generic
                       +- Consensus info -+- slot number           /  header
                      /                   +- proof of time        |
                     /                    +- future proof of time |
         +- Header -+                     +- solution             |
        /            \                                            |
Block -+              +------------ Seal -+- public key           |
        \             \                   +- signature            |
         \             \                                         -+
          |             +--- Child shard blocks -+
          |              \                        \
          |               \                        +- [ block root ]
          |                |
          |                |
          |                +- Consensus parameters -+- solution range
          |                                         +- pot slot iterations
          |                                         +- Option<super segment root>
          |                                         +- Option<next solution range>
          |                                         +- Option<{
          |                                             pot parameters change
          |                                               -+- slot
          |                                                +- slot iterations
          +- Body ---+                                     +- entropy
                      \                                }>
                       +--- Own segment roots --+
                        \                        \
                         \                        +- [ segment root ]
                          \
                           +- Intermediare shard segment roots -+
                            \                                    \
                             |                                    +- [
                             |                                        own segment roots..,
                             |                                        child segment roots..
                             |                                       ]
                              \
                               +---- Intermediare shard block headers -+
                                \                                       \
                                 \                                       +- [ block header ]
                                  +- PoT checkpoints -+
                                                       \
                                                        +- [ checkpoint ]
```

As you can see, all three block types share some similarities, but also have unique differences dictated by their role.

Now if we need to prove that something was stored in leaf shard, we can do that by generating proofs for the following
path:

```goat
Beacon chain block root --> Child shard blocks --> Intermediate shard block root -+
                                                                                  |
    +-- State root <-- Result <-- Leaf shard block root <-- Child shard blocks <--+
    | 
    +-> State item
```

This way we can reach any piece of block header or body or anything stored in a state of any block. Not only that,
information submitted from leaf shards to the intermediate shards and from intermediate shard to the beacon chain is
verifiable for integrity (that segment roots correspond to the header). Moreover, since all block roots are added to the
MMR, any historical information can also be proven/verified as well. It trees all the way down!

The whole structure and decoding ability was introduced in [PR 245] with follow-up fixes in [PR 246]. There are no
builders or owned versions of the data structures yet, but it will be easier to have them now that the structure is
known.

[PR 245]: https://github.com/nazar-pc/abundance/pull/245

[PR 246]: https://github.com/nazar-pc/abundance/pull/246

There will be more changes to this in the future once/if we have to deal with data availability and potential
misbehavior, but this should be sufficient for now.

## Block root?

You may notice that instead of "block hash" I used "block root." That is simply a reflection of the fact that a header
in itself is a Merkle Tree. Our headers are larger than those in blockchains like Bitcoin, so it is desirable to
compress the size of the proof when only a small part of it is needed (like timestamp).

## Upcoming plans

I hope you appreciate the ASCII art, I spent a non-negligible amount of time formatting it 😅.

With block layout in this state, I'll need to write even more boilerplate for builder and owned versions of data
structures. Once that is done, I'll be back to consensus verification in an attempt to get primitive blockchain
going. There are still some open questions around plotting, but I think we have a pretty good intuition with Alfonso for
how to approach it.

We post even more research updates on our [Zulip] as they happen, including meeting notes from out 1:1s with Alfonso.

[Zulip]: https://abundance.zulipchat.com/

With that, I'll see you next time with more updates and maybe even more ASCII art 😆.
