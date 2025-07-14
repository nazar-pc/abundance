---
title: A Scheduled Approach to Plot Lifecycle Management
date: 2025-07-14
draft: false
description: A new model for plot and sector lifecycle management and a draft spec in progress
tags: [status-update, consensus]
authors: [adlrocha]
---

{{< katex >}}

This week has been mainly focused on refining a bit the design for plot identification and sector
expiration. I think that I finally have a model with which I am comfortable with, and that I think
solves all of our previous problems. Nazar had this idea to drastically simplify how plot IDs were
derived, and how sectors were linked to plots. The high-level idea made sense, but there were still
some details that weren't clear. This week I managed to come up with a design that I think satisfies
all of our requirements.

Apart from all of this work around plot and sector lifecycle management, I also started working on a
draft specification for the protocol that fits all the pieces together. Our protocol builds upon the
Subspace protocol, so I am using the existing specification as a base and extending it with all the
sharding specifics. I can't wait to share the first draft with you all for feedback and suggestions.

<!--more-->

## A Scheduled Approach to Plot Lifecycle Management

The core of this proposal is based on the simplification idea for sector expiration and plot
identification from this [zulip discussion]. The basic idea is to decouple the long-term shard
assignment from the specific, moment-in-time `history_size` a farmer commits a sector to. Instead of
binding a plot to a single `history_size`, we bind it to a **Plot Epoch**, which is a large,
predefined range of history. This allows a farmer to continuously re-plot expiring sectors with new
history while remaining in the same shard for a predictable, extended period.

[zulip-discussion]:
  https://abundance.zulipchat.com/#narrow/channel/495788-research/topic/A.20radically.20simple.20farmer.20allocation.2Fsector.20expiration/with/527212623

The operation of the protocol is based on the concept of a **Plot Epoch (`PLOT_EPOCH`):** which is a
new global protocol parameter that defines a fixed length of blockchain history, measured in
segments. For example, `PLOT_EPOCH` could be set to `100,000` segments. This parameter governs the
cadence of plot turnover and mandatory shard re-shuffling for the entire network.

A farmer's assignment to a shard is determined not by the exact `history_size` they are plotting,
but by the epoch that history falls into.

- **History Class Calculation:** When a farmer creates a sector, they commit to a
  `committed_history_size`. From this, a `history_class` is derived:

```
  history_class = floor(PLOT_EPOCH / committed_history_size​)
```

- **Plot ID Derivation:** The stable `plot_id` is then created by hashing the farmer's public key
  with this class. This is the crucial step that provides stability.

```
  plot_id = hash(public_key_hash, history_class)
```

- **Shard Allocation:** This `plot_id` is then used as the input to the Verifiable Random Function
  (VRF) that determines the shard assignment.

```
  assigned_shard=vrf_output_to_shard(VRF(plot_id,randomness))
```

Because `history_class` remains the same for all history sizes within a large `PLOT_EPOCH`, the
`plot_id` and the resulting shard assignment are stable for all sectors within that history class.

#### Sector and Plot Identification

Sectors must still be uniquely and verifiably tied to the specific history they contain.

- **Sector ID:** The `sector_id` is then derived simply from this `plot_id` and the sector's index
  within the plot.

```
  sector_id=hash(plot_id, sector_index)
```

This hierarchical ID structure ensures that a farmer's shard assignment is stable for the duration
of an epoch, while still enforcing that each sector is uniquely tied to its specific
`committed_history_size` for verification and expiration purposes.

### Expiration and Re-Plotting Cadence

This model creates a natural and predictable lifecycle for farmers, directly addressing the
re-plotting concerns.

**Farmer Lifecycle Example:**

1.  **Joining (Epoch `k`):** A farmer starts plotting when the current history size is `H_1`. They
    calculate `history_class_k = floor(H_1 / PLOT_EPOCH)`. This determines their corresponding
    `plot_id` and assigns them to `plot_id` for the entire duration of Epoch `k`. They fill their
    drive with sectors committed to `H_1`.

2.  **Mid-Epoch Maintenance:** As time passes and the chain grows to history size `H_2`, their
    initial sectors from `H_1` begin to expire probabilistically. The farmer uses the newly freed
    space to plot new sectors committed to `H_2`. Since `floor(H_2 / PLOT_EPOCH)` is still
    `history_class_k`, their `plot_id` does not change, and they **remain in `plot_id`**. They are
    effectively maintaining a single plot within a single shard.

3.  **Epoch Turnover (Transition to `k+1`):** The blockchain history eventually surpasses
    `(history_class_k + 1) * PLOT_EPOCH`. At this point, any new sectors the farmer wants to plot
    must use a `committed_history_size`, `H_3`, that results in a new class: `history_class_{k+1}`.
    This forces the creation of a new `plot_id` and triggers a **mandatory re-plotting** of sectors
    into new plot for the new history class.

This creates a staggered, network-wide re-shuffling that is deterministic and tied to the growth of
the chain, rather than individual farmer decisions.

### Benefits and Parameter Tuning

This scheduled approach provides significant advantages over the previous "history window ranges"
and the simple `hash(pk, history_size)` models that I introduced the past few weeks.

- **Simplicity:** The logic is straightforward, relying on a single new parameter (`PLOT_EPOCH`) and
  simple integer arithmetic, avoiding complex range management.

- **Farmer Quality of Life:** Farmers have a stable shard assignment for long periods, reducing
  network overhead and operational complexity. They can focus on maintaining one logical plot in one
  shard.

- **Security:** It prevents the gaming attack where a farmer chooses a convenient `history_size` to
  manipulate their shard assignment. The shard is fixed for the entire epoch.

- **Controlled Dishonest Advantage:** A dishonest farmer might try to maintain valid sectors from
  two different history points within the same epoch (e.g., the very beginning and the very end) to
  temporarily increase their effective plot size. The `PLOT_EPOCH` parameter must be tuned to
  mitigate this. It should be set to a value that aligns with the probabilistic sector lifetime. For
  instance, if `PLOT_EPOCH` is roughly `1.5x` to `2x` the average sector lifespan, by the time a
  farmer could start a second plot at the end of the epoch, the first one would be significantly or
  entirely expired, keeping the maximum advantage close to the desired `~2x` bound, and often much
  less in practice.

The ideal value for `PLOT_EPOCH` requires balancing farmer stability against the security constraint
of limiting this advantage, and it can be determined by modeling against the sector expiration
probabilities.

### A model for parameter tuning

Let's define the key parameters of the system:

- \\(E\\): The `plot_epoch` size, measured in number of history segments. **This is the value we
  want to determine.**

- \\(L\_{min​}\\): The `MIN_SECTOR_LIFETIME`, a protocol constant representing the guaranteed
  minimum lifetime of any sector.

- \\(H\_{s​}\\): The `history_size` of the blockchain at the time a sector is plotted.

- \\(L*{avg​}(H*{s​})\\): The average lifetime of a sector plotted at history size \\(H\_{s​}\\).
  Based on the Subspace expiration logic, this is:

$$
  L\*{avg​}(H\_{s​})=L\_{min​}+2H\_{s​}
$$

- \\(\alpha\\): The maximum acceptable plot size multiplier for a dishonest farmer. This is our
  target security threshold (e.g., \\(\alpha=1.8\\) means we tolerate at most an 80% temporary
  inflation of a dishonest farmer's plot size).

- \\(\alpha\_{actual}(H_s,E)\\): The actual security multiplier a dishonest farmer can achieve at a
  given history size \\(H\_{s}\\) with a chosen epoch length \\(E\\).

#### Modeling the Dishonest Farmer Advantage

The primary security risk is a farmer plotting their storage at the beginning of an epoch and again
at the very end, temporarily farming with more than their actual pledged space. The maximum
advantage is achieved at the moment the epoch transitions.

The effective plot size multiplier for a dishonest farmer who plots at the beginning of an epoch
(starting at Hs​) and again at the end can be calculated as:

$$
\alpha_{actual}(H_s, E) = 2 - \frac{E - L_{min}}{4H_s}
$$

_(This formula is valid for \\(E \le L\_{min}\\)​. It represents 1 (for the new plot) + the fraction
of the old plot that has survived after \\(E\\) segments)._

#### Modeling Farmer Stability (The Churn Factor)

From a farmer's perspective, an ideal epoch length is related to the useful lifetime of their plot.
If the epoch is too short, they are forced to replot sectors in a new plot very frequently. If it's
too long, they will may need to entirely re-plot their storage multiple times for the same plot
(opening the window for potential attacks).

We can model this relationship with a **Farmer Churn Factor**, \\(C\\). This factor represents the
desired number of times a farmer should have to fully re-plot their storage (on average) within a
single epoch.

- \\(C=1\\): The epoch length is equal to the average lifetime of a plot. This is a balanced choice.

- \\(C>1\\): The epoch is longer than the average plot lifetime. This provides more stability but
  means a farmer will perform more re-plotting work _within_ an epoch.

- \\(C<1\\): The epoch is shorter than the average plot lifetime. This increases network agility and
  re-shuffling frequency at the cost of more overhead for farmers.

#### The Optimization Model

To make the `plot_epoch` a single, constant value, we must choose a **Reference History Size**,
\\(H\_{ref}\\), to anchor our calculations. This should be a value representing a mature state of
the network (e.g., the number of segments produced after one or two years of operation).

We can now define our `plot_epoch`, \\(E\\), based on our desired farmer experience (\\(C\\)) at
this reference point:

$$
E = C \times L_{avg}(H_{ref}) = C \times (L_{min} + 2H_{ref})
$$

By substituting this definition of \\(E\\) back into our security model, we get a final equation
that describes the actual security level, \\(\alpha\_{actual}(H_s)\\), at any point in the
blockchain's history \\(H_s\\):

$$
\alpha_{actual}(H_s) = 2 - \frac{C(L_{min} + 2H_{ref}) - L_{min}}{4H_s}
$$

$$
\alpha_{actual}(H_s) = 2 - \frac{(C-1)L_{min} + 2CH_{ref}}{4H_s}
$$

### An opportunity to simplify sector expiration?

Sector expiration has been the main thing messing with me when thinking about the design of this
part of the protocol. With this new approach to bind sectors into specific plots we may be able to
conceptually simplify the existing formula for sector expiration to make it more intuitive. The
logic is fundamentally sound.

Essentially, the current

**`Expiration Point = Base Lifetime + Random Additional Lifetime`**

Where:

- **Base Lifetime** = `history_size + MIN_SECTOR_LIFETIME`

- **Random Additional Lifetime** = A random value between 0 and `3 * history_size`

This conceptual model preserves all the benefits of the original design while being easier to reason
about. The current implementation, while verbose, appears to be a necessary complexity to ensure
long-term network health and fairness. What I want to do next is to figure out how to simplify this
conceptual model into a more intuitive formula that can be easily understood and reasoned about on
top of the current lifecycle management design. (_Worst case, we can keep the current sector
expiration, as it wouldn't break in any way with this new design._)

## Spec in progress and next steps

The _"protocol overview"_ section of a specification is always the harder for me. I try to give a
high-level overview of the operation of the protocl that gives readers a core intuition of how all
the piece fit together. This way, one can have a clear mental model for when we jump into the
low-level details (where is easy to get lost into the weeds).

As mentioned above, this week I started working on the specification for the protocol, and I already
managed to have a protocol overview section that I am happy with. I hope to make more progress on it
in the coming days, and to figure a way to start sharing small pieces of it so I can start getting
feedback and improvement suggestions.

Throughout all of my more theoretical work from the past few weeks, I have been sharing theoretical
models for different parts of the protocol, but I haven't shared the specific protocol parameters
for the various components (e.g. the optimal `PLOT_EPOCH` value). As part of my spec'ing efforts, I
am hoping to also recommend specific values for these parameters based on the theoretical models and
target system requirements. Hopefully, this will provide a more concrete foundation for the protocol
and help guide implementation efforts.

On another personal note, next week I will be giving a talk at the
[Web3Summit in Berlin](https://web3summit.com/), so if you are around and you want to chat about
this project, blockchain scalability, or honestly anything web3-related, feel free to reach out to
me. I will be around the event and would love to meet you in person. Looking at the image, I can see
the mathematical formulas and help you create proper LaTeX markdown. Here's the corrected version:
Looking at the image, here's the corrected LaTeX markdown:
