---
title: Modelling farmer membership allocation
date: 2025-06-09
draft: false
description: Finding the best way to assign farmers to shards.
tags: [status-update, consensus]
authors: [adlrocha]
---

Last week I shared a high-level of how I was thinking farmer membership selection should work. After
some discussions early in the week, we realised there were still some blind spots and attacks that
we weren't protecting against (or if we were, we didn't have an objective measure of how robust they
were). Thus, this week has been exclusively focused on modeling the membership selection protocol so
we can reason objectively about its design.

<!--more-->

## Modelling farmer membership selection

I was half-way in the process of writing this update when I realised that I was just paraphrasing a
lot of the text that was already part of [PR277](https://github.com/nazar-pc/abundance/pull/277).
This PR presents a draft proposal for the membership selection protocol, along with an analysis of
the security guarantees of the protocol for different kinds of adversary, mainly static and fully
adaptive adversaries.

This set the groundwork to identify the optimal protocol parameters to ensure the security of the
allocation protocol, and to what extent it is technical feasible. And to help with this process,
you've probably seen that as part of the PR I've included a really simple Python script that allows
you to input different parameters and assumptions into the model to see how the security and
feasibility of the protocol changes. The script even includes a simple estimation of the economic
cost required for a farmer to pull off an attack with high probability.

The following image shows the output of a sample execution of the aforementioned script (do not pay
attention at the specific numbers, these are just some preliminary tests I've been doing).

<p align="center">
<img alt="Sample execution of the modelling script" src="allocation_script.png">
</p>

## Useful paper and next steps

One of the key issues that were bugging me when working on the allocation mechanism was the
modelling of the fully adaptive adversary. I had the intuition of how to protect against it, but I
didn't know how to model it. Fortunately, Nazar pointed me once again to the right place to unblock
me. When I described to him what I was trying to design to protect against a specific attack from an
adaptive adversary he mentioned that it reminded him of the
[Free2Shard: Adaptive-adversary-resistant sharding via Dynamic Self Allocation](https://arxiv.org/abs/2005.09610).
Coincidentally, I read this paper when it was released but completely forget about it, and it
includes a really nice analysis of fully adaptive adversaries and how their Free2Shard proposal can
protect against it. This paper gave me all the tools that I needed for the analysis. I highly
recommend everyone to give it a quick read, as it also includes a nice introduction where it
presents the different approaches and architectures used to implement sharded blockchains, and the
benefits and disadvantages of each approach. It can give you a good intuition of the challenges
behind these designs, and where our proposal fits in.

So what's next? I'll keep having a few back-and-forths with Nazar to understand the technical
feasibility of this allocation protocol by running different scenarios through my script, and I want
to move next to detailing the specific operation of farming and block proposal in shards. Let's see
how this goes... see you next week!
