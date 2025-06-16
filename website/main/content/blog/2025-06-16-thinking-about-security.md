---
title: Thinking about the overall security of the system
date: 2025-06-16
draft: false
description: Identifying any existential risks to the system and how to mitigate them.
tags: [ status-update, consensus ]
authors: [ adlrocha ]
---

{{< katex >}}

This week has been all about objectively assessing the security of the system. After all of the work
around the membership allocation protocol and its security there are still two questions that we
need to answer to understand the feasibility of the protocol: (i) what is the security bound of the
protocol as a whole (from beacon chain to shards), and (ii) how can we ensure that plots are
uniquely identified and that farmers cannot cheat by committing the same plot to different history
sizes to try and game the shard allocation mechanism.

<!--more-->

## Computing the security bound of the protocol

The beacon chain and all shards in the system run a longest chain consensus. All farmers are
dedicating their storage to protect the beacon chain. From this, we can easily derive that as long
as there's at least 51% of the storage in the system that is honest, the beacon chain will be
secure. Unfortunately, this is not the real security bound for the protocol, as we want to
understand what is the minimum amount of honest storage that we need to ens as a whole (from beacon
chain to shards)ure for all shards to be honest with high probability. The overall security bound,
thus, will be determined by the maximum proportion of malicious nodes such that all shards are
secure with a high probability. This means we need to ensure that the probability of any shard
having 50% or more malicious farmers is very low.

We make the following assumptions to compute this probability:

- Total Malicious Nodes: Let $ p $ be the proportion of malicious nodes in the entire system (i.e.,
  among all farmers). So, if $ N $ is the total number of farmers, then $ pN $ are malicious.
- Beacon Chain Security: The beacon chain is secure as long as less than 50% of its participants are
  malicious.
- Shard Security: A shard is secure if the proportion of malicious farmers within that shard is less
  than 50%.
- Uniform and Random Allocation: Each farmer (malicious or honest) has an equal probability of being
  assigned to any given shard, assuming all of them have pledged the same amount of storage. This is
  a safe assumptions considering the fact that we are considering plots (and not farmers) as the
  basic unit of membership allocation.
- Large Number of Farmers and Shards: For statistical analysis, we'll assume a sufficiently large
  number of farmers and shards so that we can use probabilistic approximations. In the script I was
  working on last week we can refine these numbers to evaluate the impact of the number of nodes and
  shards.

We base our analysis on the [Chernoff bound](https://en.wikipedia.org/wiki/Chernoff_bound), which
provides a way to estimate the probability of deviations from the expected value in a binomial
distribution.

> Note: For this analysis I am using farmer and plot indistinctly as if each farmer had a single
> plot of maximum size. If you recall from my previous updates, and as described above, we are
> considering plots and not farmers as the minimal unit of membership allocation. Farmers are free
> to decide how they organise their storage into plots (although rationally, they will try to fit as
> much storage as possible into a single plot to reduce the number of shards they need to sync
> with).

Let's consider a single shard. Suppose there are $ S $ shards in total, and $ N $ total farmers. On
average, each shard will have $ N/S $ farmers. Let $ n_k $ be the number of farmers in shard $ k $.
Due to random allocation, $ n_k $ will vary, but for large $ N $ and $ S $, it will be close to $
N/S $.

Let $ X_i $ be an indicator random variable for farmer $ i $ being malicious

For a specific shard $ k $, let $ M_k $ be the number of malicious farmers in that shard. We want $
M_k / n_k < 0.5 $.

Since farmers are allocated uniformly and randomly, the distribution of malicious farmers within a
shard can be modeled by a binomial distribution (as discussed in
[PR277](https://github.com/nazar-pc/abundance/pull/277)). If a shard has $ n $ farmers, the number
of malicious farmers $ M $ in that shard follows $ B(n, p) $.

We are interested in the probability that $ M/n \geq 0.5 $. This is $ P(M \geq 0.5n) $.

For a system to be secure, all shards must be secure. This means we want the probability of at least
one shard being insecure to be very small.

$ P(\text{System Insecure}) = P(\exists k \text{ such that } M_k / n_k \geq 0.5) $

Using the union bound, we can say:

$ P(\text{System Insecure}) \leq \sum\_{k=1}^S P(M_k / n_k \geq 0.5) $

If we assume all shards have approximately $ n = N/S $ farmers, then:

$ P(\text{System Insecure}) \leq S \cdot P(M/n \geq 0.5) $

Now, we need to bound $ P(M/n \geq 0.5) $. Since $ E[M/n] = p $, and we want $ M/n < 0.5 $, we need
$
p $ to be significantly less than 0.5.

We can use a Chernoff bound. For a sum of independent Bernoulli random variables, if $ X_1, \dots,
X_n $ are i.i.d. Bernoulli with $ P(X_i = 1) = p $, and $ X = \sum X_i $, then for $ \delta > 0 $:

$ P(X \geq (1+\delta)np) \leq e^{-np\delta^2 / 3} $

In our case, we are interested in the upper tail, but when $ p < 0.5 $, we are interested in the
probability that $ M/n $ exceeds its mean. Let's rephrase it. We are looking for the probability
that the observed proportion of malicious nodes $ (M/n) $ is $ \geq 0.5 $, given the true proportion
is $ p $.

We set $ (1+\delta)p = 0.5 $, so $ \delta = (0.5/p) - 1 $. This bound is for $ \delta > 0 $, which
means $ 0.5 > p $. If $ p \geq 0.5 $, the probability of a shard being insecure becomes very high.

So, $ P(M/n \geq 0.5) \leq e^{-np((0.5/p)-1)^2 / 3} = e^{-n(0.5-p)^2 / (3p)} $

Let $ \epsilon = 0.5 - p $. This is the "security margin" for each shard.
$ P(M/n \geq 0.5) \leq
e^{-n\epsilon^2 / (3p)} $

Now, we need $ S \cdot e^{-n\epsilon^2 / (3p)} $ to be very small (e.g., $ 10^{-6} $ or $ 10^{-9}
$).

To provide a general number, we need to define the acceptable probability of an insecure system. Let
this be $ \alpha $.

The condition for system security is roughly:

$ \frac{N}{S} \cdot e^{-n(0.5-p)^2 / (3p)} < \alpha $

Where:

- $ N $: **Total number of farmers** in the system.
- $ n $: **Average number of farmers per shard** ($ n=N/S $, where $ S $ is the number of shards).
- $ p $: **Proportion of malicious farmers** in the entire system.
- $ \alpha $: **Acceptable probability of the overall system being insecure** (e.g., $ 10^{-6} $ for
  very high security).

You would then solve for $ p $. As shown in the example, this often leads to a quadratic inequality.

## Translating it into numbers

Based on the analysis, the percentage of malicious nodes ($ p $) that the overall system can afford
will be **significantly less than 50%**. The exact value depends critically on:

- The **total number of farmers** ($ N $) in the system.
- The **number of shards** ($ S
  $) you choose, which determines the average number of farmers per shard ($ n $).
- The **desired probability of the overall system being secure** ($ \alpha $).

As a rule of thumb, to achieve a high degree of confidence that all shards remain secure, $ p $ will
likely need to be in the range of **30-45%**, depending on the specific parameters ($ N, S, \alpha
$). It will always be a value smaller than 50% and decrease as the number of shards increases (for a
fixed total number of farmers) or as the desired security level increases.

To get the specific bound we need to choose the specific parameters desired ($ N, S, $ and $ \alpha
$) and then calculate the $ p $ value using the Chernoff bound (or a similar concentration
inequality) to get a precise answer for your design.

If we, for instance, consider total number of farmers $ N = 1000
$, number of shards $ S = 100 $,
and an average number of farmers per shard $ n=N/S=10,000 $ with a security level of $ \alpha =
10^{-2} (1\%)$,
we get that $ p $ should be approximately less than 46.42%.

The trade-off between the number of shards (which improves scalability) and the security threshold
($ p $) is clear, and it aligns with our intuition so far. More shards mean each shard has fewer
farmers (for a fixed total number of farmers), making individual shards more susceptible to higher
concentrations of malicious nodes.

## Realising plot Uniqueness

In Subspace, plots do not have a unique identifier, and are sectors (that conform a plot) the ones
uniquely identified. This is the way in which is done: _sectors are indexed sequentially and for
each sector their ID is derived through
`sector_id = keyed_hash(public_key_hash, sector_index || history_size)`, where `sector_index` is the
sector index in the plot and `history_size` is the current history size at the time of sector
creation._ This means that farmers can easily create sectors with the same sector index that commit
to different history sizes so they can keep them around to include them in a plot when is convenient
for them to achieve the desired allocation.

To prevent this attack, the idea is to assign a unique identifier to each plot that bounds the plot
to a specific window of history sizes, and unique assign sectors to plots based on this (i.e.
sectors from a plot need to be committed to a specific history size in the range bound by the plot).
Additionally, the specific size of the history size window allowed for a plot will depend on the
maximum size that the farmer want to assign to that plot. This way, we limit the surface of the
attack were a farmer tries to create different sectors with the same sector ID in a plot to try and
bias the allocation of shards in their favour.

Thus, this is how the plot and sector IDs are derived and bound together:

```
plot_id = keyed_hash(public_key_hash, max_plot_size || min_history_size || max_history_size)

sector_id = keyed_hash(plot_id, sector_index || history_size)
```

With this approach we are:

- Preventing Double-Committing Sectors: A farmer cannot commit the "same" sector ID to different
  `history_size` values and then choose the most convenient one for membership. Why? Because the
  `history_size` is part of the `sector_id`. If `history_size_A` is used, it produces `sector_id_A`.
  If `history_size_B` is used, it produces `sector_id_B`. These are distinct. If both
  `history_size_A` and `history_size_B` fall within the `[min_history_size, max_history_size]` range
  of the _same_ `plot_id`, the farmer would effectively be storing two _different_ sectors (with
  different piece selections) at the same `sector_index` within that plot, which is impossible. Each
  `(plot_id, sector_index)` pair uniquely maps to a single valid `sector_id` based on its
  `history_size` at creation time.
- Ensuring Verifiability: All parameters necessary for `plot_id` and `sector_id` derivation are
  either public (on-chain registered `plot_id`s) or committed by the farmer in proofs
  (`sector_index`, `history_size`). This allows full independent verification by any node. The
  `max_plot_size` parameter in the `plot_id` ensures that farmers must commit to a specific maximum
  plot capacity upfront, preventing them from retroactively adjusting plot boundaries to game the
  allocation mechanism. If we don't want to make the history window size have to be shared in
  plaintext off-band, and we want the verification to be self-contained, we can force a specific
  size for the history window (so it is enough with sharing the beginning of the history window), or
  we can make a commitment on the history window size so that given the history size that a sector
  is committed to, we can succinctly verify that the sector is valid for that history size of the
  plot. This verifiability will allow us to include membership allocation proofs in the block
  proposal to verify that the farmer is entitled to propose a block in that shard.

## What's next?

We are making pretty good progress on surfacing all the existential risks to the system and
modelling it in a way that allows us to clearly reason about the different trade-offs we may incur
in. Unless something unexpected comes up, my focus for the week is to:

- Embed the high-level plot ID proposal that I shared into the membership allocation protocol for
  verification.
- With this, continue modeling the security of membership allocation and the security of the
  protocol as a whole so we can have a clearer model of this looks like, and if this design satisfy
  our security requirements without the need of fraud proofs or off-band mechanisms.
- Finally, I want to spend some time fleshing out the segment submission description from
  [PR267](https://github.com/nazar-pc/abundance/pull/267) after the most recent feedback to start
  integrating into the block proposal in shards (as Nazar has already made good progress to make
  this work practically and it would be great to unblock him).
