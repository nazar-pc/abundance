---
title: The beginning of a Spec
date: 2025-04-21
draft: false
description: An artifact to foster design discussions and guide the implementation of prototypes.
tags: [status-update, consensus]
authors: [adlrocha]
---

Over the past weeks, my updates have highlighted many of the ideas emerging from our open design
discussions. Now that we have a clearer direction for the design, I wanted to consolidate these
ideas into a draft spec. This will serve as a foundation for implementing the first few prototypes,
while also providing a structured way to gather feedback and uncover potential blind spots. I expect
this spec to suffer significant changes, but it felt like the perfect way to consolidate the ideas,
get feedback from the community, and unblock Nazar in case he wants to start prototyping some of the
ideas we've been discussing.

<!--more-->

## Where to find the spec and the first drafts

[PR192](https://github.com/nazar-pc/abundance/pull/192) adds a `README` to the `spec` directory
including a description of all the components of the spec that have been drafted, and that I am
currently working on. In this PR you can already find a first draft of the spec for the sharded
archiving protocol that we've been discussing the past few weeks. Additionally,
[PR193](https://github.com/nazar-pc/abundance/pull/193) includes the draft for the data availability
sampling protocol.

These drafts are not final, and may still require several rounds of feedback and iterations, so
don't expect these PRs to be immediately merged. However, you can already follow the progress
through these open PRs, as well as use them to provide direct feedback about the designs. Even more,
by giving us feedback in the PRs, your suggestion can be directly incorporated into the spec as we
continue refining it (otherwise, you know that you have the
[Zulip server](https://abundance.zulipchat.com/) available for discussions and feedback).

## What's next?

I decided to keep this update brief so you can focus on reading the spec and providing feedback. You
will find there all the new ideas and progress from the past week. Let me hype it a bit by giving
you the highlights and main changes from last week:

- Instead of using super commitments as a way to recursively commit segments to the global history
  of the beacon chain, we've simplified the protocol to allow shards, independently of their level
  in the hierarchy, to directly commit their segments to the global history in the beacon chain.
- Super segments are now created by the beacon chain to allow light clients to easily verify that
  shard segments belong to the global history in the beacon chain, given the right witnesses.
- The data availability sampling protocol specification now includes a more detailed description of
  the end-to-end of the protocol, including what happens if a block (or a segment) are flagged as
  unavailable, and how farmers in the beacon chain handle this event.

This week I hope to start collecting a lot of feedback for the draft specs so I can iterate on the
design and add more details and improvements where needed. In parallel, I am already figuring out
how sharded plotting will work on top of sharded archiving. Fortunately, all the work Nazar has been
doing to
[replace KZG by Merkle proofs](https://abundance.build/blog/2025-04-20-very-fast-archiving/) already
handles (hopefully) a lot of the heavy lifting of what will be needed for plotting.
