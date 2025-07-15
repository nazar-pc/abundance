---
title: A Deterministic Mapping for Plot Lifecycle Management
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

<!--more-->

Apart from all of this work around plot and sector lifecycle management, I also started working on a
draft specification for the protocol that fits all the pieces together. Our protocol builds upon the
Subspace protocol, so I am using the existing specification as a base and extending it with all the
sharding specifics. I can't wait to share the first draft with you all for feedback and suggestions.

## A Deterministic Mapping for Plot Lifecycle Management

The core of this proposal is based on the simplification idea for sector expiration and plot
identification from this [zulip discussion]. The basic idea is to simplify plot identification by
removing scheduled, network-wide epochs. Instead, we introduce a **deterministic, cyclical mapping
function**. This approach decouples long-term shard assignment from the specific `history_size` by
mapping any `history_size` to a fixed set of "history classes". This allows a farmer to continuously
update sectors with new history while having the flexibility to remain in the same shard, making a
`plot epoch` parameter unnecessary. This also limits the number of sectors that can be created in
parallel with the same sector index and different committed history sizes (artifically increasing
the storage of a plot).

The operation of the protocol is based on the concept of a **History Modulo (`HISTORY_MODULO`)**, a
new global protocol parameter that defines the total number of unique history classes. For example,
`HISTORY_MODULO` could be set to `2048`. This parameter governs the trade-off between farmer
flexibility and shard selection potential.

A farmer's assignment to a shard is determined not by the exact `history_size` they are plotting,
but by the class that history maps to.

- **History Class Calculation:** When a farmer creates a sector, they commit to a
  `committed_history_size`. From this, a `history_class` is derived using a simple modulo operation:
  ```
  history_class = committed_history_size % HISTORY_MODULO
  ```
- **Plot ID Derivation:** The stable `plot_id` is then created by hashing the farmer's public key
  with this class. This is the crucial step that provides stability.
  ```
  plot_id = hash(public_key_hash, history_class)
  ```
- **Shard Allocation:** This `plot_id` is then used as the input to the Verifiable Random Function
  (VRF) that determines the shard assignment.
  ```
  assigned_shard = vrf_output_to_shard(VRF(plot_id, randomness))
  ```

Because `history_class` is the result of a modulo operation, many different `history_size` values
will map to the same class. This ensures the `plot_id` and the resulting shard assignment can remain
stable even as the farmer plots new history.

#### Sector and Plot Identification

Sectors must still be uniquely and verifiably tied to the specific history they contain.

- **Sector ID:** The `sector_id` is derived simply from this `plot_id` and the sector's index within
  the plot.
  ```
  sector_id = hash(plot_id, sector_index)
  ```

This hierarchical ID structure ensures that a farmer's shard assignment can be stable, while still
enforcing that each sector is uniquely tied to its specific `committed_history_size` for
verification and expiration.

### Expiration and Re-Plotting Cadence

This model creates a flexible and predictable lifecycle for farmers, giving them direct control over
their re-plotting strategy.

**Farmer Lifecycle Example:**

1.  **Joining:** A farmer starts plotting when the current history size is `H_1`. They calculate
    `history_class_1 = H_1 % HISTORY_MODULO`. This determines a `plot_id_1` and assigns them to a
    corresponding shard. They begin filling their drive with sectors committed to `H_1`.

2.  **Maintenance:** As time passes and the chain grows to a history size of `H_2`, their initial
    sectors from `H_1` begin to expire. The farmer uses the newly freed space to plot new sectors.
    They now have a choice:

- **Re-use Existing Plot:** The farmer can choose to remain in the same plot (and thus the same
  shard). To do this, they find a `committed_history_size` (`H_commit`) that is less than or equal
  to `H_2` and also satisfies `H_commit % HISTORY_MODULO == history_class_1`. A farmer can easily
  calculate the most recent history size that meets this criterion.
- **Utilize a New Plot:** Alternatively, the farmer can commit to the latest history, `H_2`. This
  will generate a new class, `history_class_2 = H_2 % HISTORY_MODULO`, creating a new `plot_id` and
  likely assigning them to a new shard.

This model removes the concept of a mandatory, network-wide "Epoch Turnover." Instead, re-shuffling
becomes a strategic choice made by the farmer.

### Benefits and Parameter Tuning

This deterministic mapping provides significant advantages:

- **Simplicity:** The logic is extremely straightforward, relying on a single new parameter
  (`HISTORY_MODULO`) and simple modulo arithmetic.
- **Farmer Quality of Life:** Farmers are never forced into a disruptive, network-wide re-plotting
  event. They have fine-grained control over which plots they maintain, reducing operational
  overhead.
- **Flexibility:** Farmers can choose to commit to a slightly older history size to preserve their
  shard assignment or switch to a new one if it is more advantageous.

The primary consideration is tuning the `HISTORY_MODULO` parameter, which involves a direct
trade-off between farmer flexibility and preventing strategic shard selection.

### A model for parameter tuning

The key to this design is selecting an optimal value for \\((M\\), the `HISTORY_MODULO`. This choice
involves balancing two competing factors: **History Lag** and **Shard Selection Power**.

Let's define the key parameters:

- \\(M\\): The `HISTORY_MODULO`, the number of distinct history classes. **This is the value we want
  to determine.**
- \\(S\\): The total number of shards in the network.
- \\(H\_{current}\\): The current `history_size` of the blockchain.

#### Modeling Farmer Flexibility (History Lag)

When a farmer wants to re-use an existing plot, they may not be able to use \\(H\_{current}\\). This
creates a "lag" between their plotted history and the most current state of the network. We can
quantify this as the **Average History Lag** (\(L_H\)):

$$
L_H = \frac{M}{2}
$$

A larger $M$ gives the farmer more classes to choose from, reducing the average lag for any given
class.

#### Modeling Security (Shard Selection Power)

A farmer could try to gain an advantage by choosing which history class to plot. With \\(M\\)
possible classes, they can generate \\(M\\) different `plot_id`s and calculate the resulting shard
for each, choosing the one that is most favorable. We can model this as the **Shard Selection
Power** (\\(P\_{select}\\)), the probability of landing in one specific target shard:

$$
P_{select}(M, S) = 1 - \left(1 - \frac{1}{S}\right)^M
$$

A smaller \\(M\\) significantly reduces this probability, making it difficult for farmers to
strategically choose their shard.

#### The Optimization Model

The goal is to choose a value for \\(M\\) that provides a good balance. For example, if a network
has \\(S=1000\\) shards and we decide that no farmer should have more than a 5% chance of selecting
a specific shard, we would solve for \\(M\\):

$$
0.05 = 1 - \left(1 - \frac{1}{1000}\right)^M
$$

$$
\ln(0.95) = M \cdot \ln(0.999) \implies M \approx 51.3
$$

Based on this, a protocol designer might choose **$M=50$**. This value would provide a low risk of
shard selection while ensuring the average history lag is only 25 segments—a very reasonable
trade-off for farmer flexibility.

### An opportunity to simplify sector expiration?

Sector expiration has been the main thing messing with me when thinking about the design of this
part of the protocol. With this new approach to bind sectors into specific plots we may be able to
conceptually simplify the existing formula for sector expiration to make it more intuitive. The
logic is fundamentally sound.

Essentially, the current model can be thought of as:

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
for the various components (e.g. the optimal `HISTORY_MODULO` value). As part of my spec'ing
efforts, I am hoping to also recommend specific values for these parameters based on the theoretical
models and target system requirements. Hopefully, this will provide a more concrete foundation for
the protocol and help guide implementation efforts.

On another personal note, next week I will be giving a talk at the
[Web3Summit in Berlin](https://web3summit.com/), so if you are around and you want to chat about
this project, blockchain scalability, or honestly anything web3-related, feel free to reach out to
me. I will be around the event and would love to meet you in person.

[zulip-discussion]:
  https://abundance.zulipchat.com/#narrow/channel/495788-research/topic/A.20radically.20simple.20farmer.20allocation.2Fsector.20expiration/with/527212623
