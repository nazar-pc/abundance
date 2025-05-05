---
title: From sharded archiving to sharded plotting
date: 2025-05-05
draft: false
description: Continue with sharded archiving spec discussions and kicking-off plotting design
tags: [status-update, consensus]
authors: [adlrocha]
---

We keep iterating on the best way to discuss and make progress on the design of the protocol. Using
issues for discussions have shown less efficient than originally expected. The inability to make
in-line threads, and having to quote every single detail of the spec that we want to discuss about
was really cumbersome. I started the
[shard block submission issue](https://github.com/nazar-pc/abundance/issues/215) as an attempt to`
start iterating the low-level details of specific protocol mechanisms in a way that is narrow enough
and easy to track, but it didn't fulfill all our needs. The solution? Creating discussion PRs that I
don't expect to get merged, but gives us all that we need to have low-level discussions about
specific parts of the protocol, track our progress, open ideas, and discussions, and have them
public so anyone can contribute or follow along.

<!--more-->

## Finalising the mechanics for sharded archiving.

Last week I've mainly focused on trying to flesh out the basic mechanics for sharded archiving,
detailing the basic data structures and proofs required to start designing the next steps in the
protocol, mainly plotting, farming, and data availability. The result of this work are these two
PRs: [PR220](https://github.com/nazar-pc/abundance/pull/220) with the end-to-end mechanics of child
shard block submission to the parent chain and the generation of global history inclusion proofs;
and [PR227](https://github.com/nazar-pc/abundance/pull/227) with the operation for segment
submission to the global history in the beacon chain, and the corresponding inclusion proofs for
segment pieces. As mentioned above, the goal for these PRs is not to get them merged as "authorative
spec artifacts" but to have constraint discussions that allows us to iterate more efficiently and
help with surfacing the requirements for prototyping them.

## Parking lot of discussions

Along with the discussions above, I also pushed in
[PR228](https://github.com/nazar-pc/abundance/pull/228), a new section on the spec discussions
directory that I called
[the parking lot](https://svprojectmanagement.com/use-the-parking-lot-method-for-easy-flowing-conversations-and-meetings)
where I am starting to collect some discussion points that we've been having sync or a sync of
things that need to be defined further in the future but that we've _"parked"_ until we have
designed more core components that we could build upon. I didn't want all this to be lost on my
meeting notes so I decided to start sharing them publicly _("hey, maybe someone has already some
ideas and can pick up these problems themselves. Ping me in Zulip or anywhere else if this is the
case")_. At some point, I am even considering pushing all my meeting notes to make them pubic,
there's a lot of good discussions and information there, but it may end up adding more noise to the
repo than anything else, so I am refraining from it for now.

## Coming next: Sharded plotting and data availability

This week I'll probably be stretched into different fronts:

- I'll try to start addressing any feedback that surfaces from the PRs with the spec of proof and
  segments mechanics of sharded archiving.
- With sharded archiving in a good place, I've started drafting the design for sharded plotting.
  While this subprotocol may not introduce big changes from how it currently works in Subspace,
  getting it right will be fundamental for the correct and efficient operation of sharded archiving
  and block proposal.
- Finally, the more progress I make into the design, the clearer it becomes that on a sharded
  architecture like ours, the data availability layer will be key to ensure the security of the
  protocol. Thus, I want to re-purpose the spec PR
  ([PR193](https://github.com/nazar-pc/abundance/pull/193)) that I started a few weeks ago with the
  high-level design into narrower discussions like I did for sharded archiving with
  [PR220](https://github.com/nazar-pc/abundance/pull/220) and
  [PR227](https://github.com/nazar-pc/abundance/pull/227).
