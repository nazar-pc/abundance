---
title: Digging deeper into sharded archiving
date: 2025-05-12
draft: false
description: Continue clearing the fog around sharded archiving and data availability
tags: [status-update, consensus]
authors: [adlrocha]
---

This week has been mainly focused on clearing the fog around shard archiving and trying to start
fleshing the low-level details for the protocol. It feels like in the past few weeks we've been
surfacing more questions than answers, and I honestly think this is a good sign. It means that we
are getting to the point where we can start to see the details of the protocol and how it will work
in practice.

<!--more-->

## Iterating on the protocol spec

If you recall from last week's update, I started
[PR220](https://github.com/nazar-pc/abundance/pull/220) with the end-to-end mechanics of child shard
block submission to the parent chain and the generation of global history inclusion proofs; and
[PR227](https://github.com/nazar-pc/abundance/pull/227) with the operation for segment generation
and commitment to the global history. Nazar has been kind (and patient) enough to have done already
a few rounds of feedback. This has really helped to dig deeper into details and flesh out the spec
of these protocols. You can refer to the discussions on those PRs if you are interested about the
details, but let me give you a few highlights after these iterations:

- We have a better sense of how the end-to-end of child block submission and their proof generation
  will look like. This is allowing us to iterate on what the final data structure for `BlockHeader`s
  will look like.
- We still don't have a good idea of how to deal with potential reorgs in the beacon chain and child
  shards. Brainstorming about the high-level of reorg handling will be one of the main focus for me
  this week.
- Regarding segment commitments, as you may see from Nazar comments in the
  [PR227](https://github.com/nazar-pc/abundance/pull/227), the spec was still a bit under-defined,
  and many details were still missing. Fortunately, after our sync from last Friday I think I have
  all the details needed to flesh out the spec. This week I am planning to re-write the whole spec
  to include all of these details before a new round of feedback.

## Discussing the source of randomness for the system

One of the big questions that have been bothering me throughout the week, and that took me down the
rabbit hole of reading about the state of the art of unbiased randomness beacons and randomness
chains was the following: _"Should we consider a PoT chain that is independent of the beacon chain
for randomness beacon generation?_. This question was triggered by a comment from Nazar in one of
the PRs that made me realise that a reorg in the beacon chain can trigger a reorg of the PoT chain.
This is caused by the fact that the PoT chain periodically injects entropy from the main chain (in
our case the beacon chain). Our system will leverage this source of randomness to sync all the
shards in the system, so any reorg of the randomness chain will inevitably cause the rest of shards
to have to reorg to accommodate the changes on past randomness slots. Additionally, Nazar was
mentioning in this [research
thread](https://abundance.zulipchat.com/#narrow/channel/495788-research/topic/Light.20client/with/516959267
"https://abundance.zulipchat.com/#narrow/channel/495788-research/topic/Light.20client/with/516959267)
some potential complications with verifying randomness in light clients.

As a result of this I started wondering if we should consider using an independent randomness chain
that is based in other cryptographic primitives that is not an AES-based VRF (which is currently the
case) and is efficient to verify, like is the case of
[drand](https://docs.drand.love/docs/cryptography/). We even
[discussed](https://abundance.zulipchat.com/#narrow/channel/495788-research/topic/Light.20client/with/517265193)
the possibility of leveraging directly the drand network as independent randomness chain.
Unfortunately, this may not be possible at this stage (as described
[here](https://abundance.zulipchat.com/#narrow/channel/495788-research/topic/Light.20client/near/517250894)).

The outcome of this discussion has been that we are going to keep the operation of the PoT chain as
it works in Subspace for now and we are going to tweak the parameters so that we minimise the
probability of a reorg.

## Following the progress

Which leads me to the next important highlight of the week: so far, the best way to follow our
progress in terms of design was to read the spec discussions PRs, or the topical discussions from
Zulip. For instance, here are a few of the discussions that have been produced from this week's
work:

- [Mechanics of child block submission to parent chain](https://abundance.zulipchat.com/#narrow/channel/495788-research/topic/Mechanics.20of.20child.20block.20submission.20to.20parent.20chain/with/517571994)
- [Whether to include timestamp in the block header or not](https://abundance.zulipchat.com/#narrow/channel/495788-research/topic/Whether.20to.20include.20timestamp.20in.20the.20block.20header.20or.20not/with/517262114)
- [OptimumP2P (RLNC) v.s. libp2p gossipsub](https://abundance.zulipchat.com/#narrow/channel/495788-research/topic/OptimumP2P.20.28RLNC.29.20v.2Es.2E.20libp2p.20gossipsub/with/517103394)
- [Address formatting](https://abundance.zulipchat.com/#narrow/channel/495768-engineering/topic/Address.20formatting/with/517323999)'
- [Light client](https://abundance.zulipchat.com/#narrow/channel/495788-research/topic/Light.20client/with/517265193)

But you've probably realised by now that I keep referring on this updates to my syncs with Nazar,
and I didn't like the fact that these weren't public and the outcomes of these meetings couldn't be
followed publicly. This is why I've started publishing my meeting notes directly as Zulip
discussions: here's
[the link to the notes from last week's meeting](https://abundance.zulipchat.com/#narrow/channel/502084-meeting-notes/topic/2025-05-09/near/517105890).
These are raw and unformatted notes that I am taking live, so expect typos and some inconsistencies,
and don't expect them to be easily readable, but at least it allows me to give you a glimpse of what
we've been chatting about in case you are interested.

## What's next

As already advanced above, my goals for this week hasn't changed significantly from last week. I
will try to continue iterating with Nazar in the spec discussion for sharded archiving so we clarify
some of my current blind spots like the handling of reorgs by child shards and the beacon chain. I
want to leave the specs for sharded archiving in a state that allows me to start new spec
discussions for sharded plotting and data availability sampling (for which I already have lots of
notes of the high-level design). That's all for now! As always, any feedback or suggestions about
the content of this post or my work overall, please hit me up!
