---
title: Super segments (part 2)
date: 2026-03-17
draft: false
description: Super segments are feature-complete
tags: [ status-update ]
authors: [ nazar-pc ]
---

Good news! Not very long since the last update, and super segments are now feature-complete.

<!--more-->

Originally, farmer's caching and replotting was driven by the archiving process of the single chain. With sharded
architecture there will be multiple individual shards archiving their segments separately, and only when confirmed they
become a part of the global history.

## Super segment creation

[PR 571] implemented the first step of the process: creation of super segments. You can open PR for more details, but
essentially there is a new consensus constant `shard_confirmation_depth` defining how many blocks on the beacon chain
are needed to confirm a shard segment (and block). There are also various super segment-related abstractions and the
client API for dealing with super segments.

[PR 571]: https://github.com/nazar-pc/abundance/pull/571

A few hacks were used to make it work in the meantime, primarily on the RPC side to make the process somewhat
transparent for the farmer.

Consensus verification started properly verifying pieces against the global history, which was previously skipped due to
anticipated changes in the protocol. The only remaining bit of work is missing sector expiration check.

## Super segment-aware farmer

[PR 579] finally removed the hacks mentioned above and made the farmer aware of super segments. Its caching and
replotting logic is now driven by the creation of super segments rather than regular segments. There are some
improvements that can be done there, and RPC layer doesn't have non-beacon chain shards to work with for now, but
overall it is close to what it should be.

[PR 579]: https://github.com/nazar-pc/abundance/pull/579

Sector expiration check was also implemented here, although I am not 100% happy with it, it doesn't look elegant that
the expiration is driven by segments, but super segment root is used for expiration derivation. On the flip side,
changing expiration to be driven by super segments while possible, is not equivalent due to super segments having an
inherently variable number of segments in them.

As a bonus, there is also now an API for re-deriving segments, which can be used for non-consensus purposes, like
mapping objects contained in blocks to their location in the global history. However, archiver's API is a bit off right
now for objects specifically and will need to be adjusted to be segment-focused rather than block-focused (or both?).

With all of that done, I was able to remove the remaining conversion hack between local and global segment indices used
during the transition period in [PR 580].

[PR 580]: https://github.com/nazar-pc/abundance/pull/580

## Upcoming plans

Relatively short update this time. In case you missed it, I [published a bunch of general-purpose crates] to
crates.io a few days ago. Give them a try and share your feedback if you have any.

[published a bunch of general-purpose crates]: ../2026-03-14-first-crates-on-crates.io

I think I'm in a mood to work some more on RISC-V interpreter next. I want to establish abstractions for non-GPRs and
add basic RV32 support now that the crates are published.

I might also refactor the Merkle Tree implementation to make it generic over the hasher. It will make code even more
hairy with more complex where bounds, but it will not only be useful for external users, but also for optimizing the
size of shard membership proofs, which currently use the full 32-byte BLAKE3 hashes, but only actually need 8-byte
hashes, which will make block headers smaller.

The next major step for consensus is to figure out the exact relationship between shard blocks on various levels. Then
I'll be ready to start writing consensus specifications explaining how everything works since right now it is not
approachable at all.

If you have any thoughts, share them on [Zulip], and I'll be writing with more updates some time soon.

[Zulip]: https://abundance.zulipchat.com/
