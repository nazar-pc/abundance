---
title: Simplifying Plot Expiration
date: 2025-07-07
draft: false
description: From history window ranges to a simpler model
tags: [status-update, consensus]
authors: [adlrocha]
---

I have to admit that I am a bit disappointed with my progress this week. If you recall from last
week's update, I started the week with a base proposal to handle the linking of sectors to plots
based on history window ranges. The idea was to limit the number of parallel sectors that could be
created in parallel, linked to the same plot, and hence allocated to the same shard. While the
approach seemed quite elegant because it didn't require any changes to how piece selection and
expiration currently works in Subspace, it turned out to be pretty complicated (even to explain) and
not the most effective solution to prevent the attack I was trying to mitigate.

<!--more-->

## A formal model to convince myself

I still thought that the idea of window ranges was a good one, so I tried to come up with a model to
understand the optimal window sizes to minimise the number of parallel sectors that a farmer could
have in parallel for the same plot. The result wasn't very encouraging, as the history size
increased, the number of sectors that could be created in parallel also increased significantly as
farmer windows grow. The situation was such that even with windows we ended up in a similar
situation to the one where we didn't have history windows and every sector was committed to a
different history size (like in the original Subspace protocol).

This was more apparent when Nazar pointed this same thing out to me on one of our syncs. For small
history sizes the solution could make sense, but as the history size we just add complexity to the
protocol without any real benefit.

## Everything should be made as simple as possible but not simpler

Let's take a step back to understand why we were doing this in the first place.

- We needed a way to link sectors to plots so that we could use plots as core unit for the
  allocation of storage space in shards.
- We wanted to prevent farmers from being able to bias or game shard membership by creating multiple
  sectors linked to the same plot committed to different history sizes but with the same sector ID.
- Sector expiration and piece selection should be designed in a way that ascribes with the above
  requirements while maintaining the basic properties (in terms of sector lifetime and history load
  balancing, respectively) of current Subspace protocol.

Fortunately, Nazar to the rescue once again! He introduced in this [Zulip
discussion][zulip-discussion] a really simple solution that solves all of the above with the only
drawback that it may require farmers to keep at least to active plots in parallel, what would
require them to sync with at least two more shards. But this may end up becoming more of a feature
than a problem.

## The Simple Solution

The key idea is that we derive a `plot_id` from the combination of `public_key_hash` and
`history_size`, and then commit individual sectors to this derived plot identity (uniquely linking
them to the plot).

Here's how it works:

1.  **Plot Identity**: `plot_id = hash(public_key_hash, history_size)`
2.  **Sector Identity**: `sector_id = hash(plot_id, sector_index)`

This approach maintains the essential properties we need:

- Each plot is effectively bound to a unique history size through the `plot_id` derivation
- Sectors within a plot share the same history size commitment
- The gaming attack is prevented because changing the history size results in a completely different
  `plot_id`

## What happens when sectors expire?

One of the key concerns I initially had about this approach was whether all sectors within a plot
would expire simultaneously, creating a massive re-plotting burden for farmers. However, as Nazar
clarified, this isn't the case at all.

The expiration mechanism remains exactly the same as in the current Subspace protocol:
**probabilistic expiration with the committed history size as an anchor**. Here's how it works:

1. We wait for a certain number of segments to pass
2. We determine expiration for each sector individually using the same probabilistic mechanism
3. Sectors expire gradually over time, not all at once

This means that even though all sectors in a plot are committed to the same history size, they don't
expire simultaneously. The expiration is still spread out probabilistically, maintaining the same
load balancing properties we have today.

## The Multi-Plot Reality

One interesting consequence of this approach is that farmers will likely end up maintaining **at
least two active plots in parallel**. Initially, I was concerned this would be a drawback, but I've
come to realize it might actually be a feature rather than a bug.

Here's the typical farmer lifecycle under this model:

1. **Initial State**: Farmer creates a plot with `plot_id_A = hash(public_key_hash, history_size_A)`
   and fills it with sectors
2. **Gradual Expiration**: Over time, sectors in the plot begin to expire probabilistically
3. **New Plot Creation**: When the first sector expires, the farmer picks a new history size
   (`history_size_B`) and creates a new plot (`plot_id_B`) for subsequent sectors
4. **Parallel Operation**: For a period, the farmer maintains both plots, syncing with multiple
   shards
5. **Transition**: Eventually, all sectors in the old plot expire, and the farmer can focus solely
   on the new plot

This creates an interesting economic decision point for farmers: **Should they dedicate the storage
space of the expiring plot to continue syncing with the allocated shard, or is it more economical to
re-plot those sectors so they only need to sync with one shard?**

## Benefits of the Simple Approach

The more I've thought about this solution, the more elegant it becomes:

- **Simplicity**: No complex window ranges or intricate sector management logic
- **Security**: Prevents the gaming attack by binding plots to specific history sizes
- **Flexibility**: Farmers can optimize their shard participation based on economic incentives
- **Compatibility**: Maintains the core properties of the existing Subspace protocol
- **Natural Load Balancing**: Forces farmers to distribute across multiple shards over time

The requirement to sync with multiple shards might actually improve network resilience by ensuring
farmers maintain connections to different parts of the network.

## Next Steps: Formal Specification

Now that we've settled on this approach, I think I have everything that I need to start writing an
end-to-end spec with all the ideas and protocol mechanisms that we've come up with in the past
months.

Having all these protocol mechanisms I've been describing consolidated into a single specification
will provide a clear roadmap for implementation while having a single source to start gathering
feedback from the broader community. I am still figuring out the best format for this spec. I am
thinking about forking the Subspace protocol spec and including all the sharding mechanisms in it,
or to create a separate one. So far I've started writing it in a separate file for now, but I may
also try the fork approach to keep everything in one place and avoid having to re-write some things.
But for any of you that have been following this updates actively, feel free to let me know your
thoughts on this, or if you already have feedback or concerns about any of the ideas presented so
far.

Until next week! I hope this one is a more productive one, as I am really excited to start gathering
feedback for all the work we've done so far.

[zulip-discussion]:
  https://abundance.zulipchat.com/#narrow/channel/495788-research/topic/A.20radically.20simple.20farmer.20allocation.2Fsector.20expiration/with/527212623
