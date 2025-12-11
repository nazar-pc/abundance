---
title: Steps towards multi-shard farming
date: 2025-12-11
draft: false
description: Preparation for upcoming support of running multiple shards at once
tags: [ status-update ]
authors: [ nazar-pc ]
---

So far the block production was basic and very similar to [Subspace], despite some key underlying differences. But that
is just an early prototype to get beacon chain running, but intermediate and leaf shards must be added into the mix,
which substantially complicate things.

[Subspace]: https://subspace.github.io/protocol-specs/docs/category/consensus

<!--more-->

I've spent a lot of time thinking about various moving pieces and merged a few PRs with various refactorings that will
be useful down the road.

In short, there are two key things that are completely unimplemented and not even described accurately yet, which I
focused on:

* shard allocation algorithm
* global archival history

## Shard allocation algorithm

The closest to public description of the algorithm is by Alfonso in [PR 277]. The PR is not 100% accurate and was
supposed to be updated, but that never happened, so I'll describe the gist of the problem and solution here.

[PR 277]:  https://github.com/nazar-pc/abundance/pull/277

In short, the goal is to have a completely permissionless algorithm, according to which farmers will be assigned to
shards, while ensuring uniform distribution of both space pledged and security (honest/adversarial farmers ratio).

The permissionless part is what causes the most trouble here: farmers should not register or explicitly announce
themselves on the network, yet they should be assigned to a shard verifiably. Moreover, farmers can't be allowed to just
pick the shard at will because attacker would easily occupy and compromise a shard, wreaking havoc since all shards are
considered to be trusted, but not fully verified by everyone.

Luckily, to participate in the consensus, farmers have to do plotting, which is a compute-intensive process. So their
plot just needs to be assigned to a shard unpredictably, in a way that forces them to replot from scratch when a
specific shard destination is desired.

While at it, the solution also considers a few hardening factors: post-quantum security (so not VRFs with elite-curve
cryptography) and some DoS protection (so it is not possible to guess which farmer is assigned to which shard
externally).

Here is the solution sketch:

* farmer derives a set of random private values (will likely be derived from the keypair), something like \(2^20\) of
  them, and builds a Merkle Tree from it
* the plotting process requires a farmer to include Merkle Tree root in the PoSpace seed when encoding sectors, tying
  sectors to the set of values
* on-chain randomness is taken at an interval (like 1 hour, depends on how fast and expensive shard sync is) to update
  shard assignment, ensuring the assignment is not static
* on-chain together with farmer's public key hash indicates which value in the above Merkle Tree is used for shard
  assignment
* when winning the puzzle, a farmer reveals the value in the set that proves shard assignment

This way we have a completely permissionless algorithm for shard assignment, which keeps shard assignment private until
a farmer won a chance to produce a block, while ensuring farmers are assigned to shards randomly and uniformly and are
forced to rotate according to a tight schedule.

The size of the plot is already capped to 65 TiB by the `SectorIndex` data type and on a large scale it does not matter,
but there is actually a way to inflate the effective plot size if a farmer commits the same sector to different history
sizes (and the number of options increases as blockchain gets bigger). This is a big problem if Subspace construction is
used without extra modifications. To solve this, I came up with a [simple solution], which essentially mixes in the
history size into the shard allocation algorithm. This way creating a sector for the same public key hash, but different
history size would result in a different shard allocation.

[simple solution]: https://abundance.zulipchat.com/#narrow/channel/495788-research/topic/A.20radically.20simple.20farmer.20allocation.2Fsector.20expiration/with/527356280

This is not difficult to implement and would work as intended, but it causes some inconveniences due to a farmer needing
to follow more than one shard per plot as they incrementally replot as well as being potentially forced to use outdated
history size just to avoid following the ever-growing number of shards. I had some conceptual ideas on how to improve
that, and Alfonso [came up with a proposal], but it is complex and unfinished. I'll run with the simple solution for
now, but if someone is interested in researching improvements, please ping me.

[came up with a proposal]: https://abundance.zulipchat.com/#narrow/channel/495788-research/topic/Logarithmic.20function.20for.20sector.20creation/with/529813082

## Global archival history

The global archival history is even more complicated than that. I do not have a complete solution for it yet, so I'll
share the high-level problem and solution.

The goal is to have each shard archive their local history independently as blocks are produced, yet to have an eventual
ordering of all segments into a single linear global history. Just like in Subspace, to verify any piece of the history,
it should be sufficient to have a compact list of roots that pieces can be verified against. The list should be so small
that it can be kept in memory even on resource-constrained clients, despite the massive global history size.

Conceptually, this means that the segments that each shard produces initially are basically incomplete. Segment roots
will be propagated up the hierarchy into the beacon chain. Once considered to be confirmed (I'll describe how exactly
later once I have the whole design worked out), a set of segment roots will form a super segment. In Subspace a piece
already includes a proof of that its record belongs to the segment root, so here we'll need to add yet another proof
that segment root belongs to a super segment.

Once a super segment is created, it defines the ordering of its segments, which assigns unique global index to
corresponding pieces that various clients can then use for piece retrieval and verification.

There are various edge-cases and annoying complications, but high-level this is the idea. One super segment root is just
32 bytes, but it commits to a bunch of individual segments, which have ~256 MB worth of blockchain history. This is a
pretty high compression ratio if there are hundreds or even thousands of segments in each super segment. This means that
even when the global history is measured in exabytes, it will be easy to store al super segment roots in memory even in
browser extensions and mobile apps.

## Progress so far

As mentioned at the beginning, a lot of time was spent recalling this information since I was last actively working on
it a few months ago and thinking about the exact steps. That said, I did some preparation already.

In [PR 465] I introduced `BlockProducer` trait, which will allow implementing multiple variants of it for different
kinds of shards. Previously the implementation was designed around producing beacon chain blocks. With shard assignment
there will be a single shared slot worker that issues challenges to the farmer. However, which shard the solution lands
on will vary on the solution, thus block producer needs to be customizable.

[PR 465]: https://github.com/nazar-pc/abundance/pull/465

In [PR 466] I removed separate ephemeral `SegmentHeadersStore` and merged its functionality into the database
implementation, so each shard can persist its segment headers independently across restarts.

[PR 466]: https://github.com/nazar-pc/abundance/pull/466

Unrelated, but something that bothered me for some time was [PR 461] where I removed one of the last remnants of
Substrate-specific code related to reward addresses. It now uses the format I described in the [Address formatting]
article, but even that is still a placeholder. I think eventually rewards will be slightly delayed with the "claim"
mechanism, so the implementation will most likely probably change, but at least there is no dependency on
Substrate-specific crates anymore.

[PR 461]: https://github.com/nazar-pc/abundance/pull/461

[Address formatting]: ../2025-05-12-address-formatting

## Upcoming plans

The immediate next step is to wire the initial version of the shard assignment logic into both farmer and solution
verification on the client side. From there I'll probably focus on the global archival history design.

Since these are something that I'd call more "boring," I'll probably start working on RISC-V VM/interpreter thingy in
the meantime too, which I expect to be more fun.

That is it for today, I'll be back with more updates in not-so-distant future. Until then [Zulip] is where you can find
me any day of the week.

[Zulip]: https://abundance.zulipchat.com/
