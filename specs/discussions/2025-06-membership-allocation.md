# Farmer membership allocation protocol

The main goal of the membership selection mechanism is to ensure the correct assignment of farmers
to specific shards in order to load balance the "power" (plotted space) in the system across all
shards maintaining an equally high and consistent level of security for each shard. This allocation
should protect against static and adaptive adversaries that may be able to take control of a shard
by concentrating their plots in a single shard affecting their security and liveness.

Below I share a proposal for a membership allocation protocol where in each point I try to share the
rationale and the analysis of why the approach has been chosen. Casual readers should be able to
grasp the operation of the protocol by just reading the headlines of each bullet point:

## Preamble

- Farmer plots are the unique unit of membership (and identity) for the allocation protocol

  - Membership allocation in PoS systems where the list of available validators at a given epoch is
    known consider a single identity per validator, being their weight proportional to their stake
    in the system. In our case, the system is completely permissionless, and we don't have a priori
    knowledge of the list of validators at a given epoch. Hence, a farmer should be able to compute
    their shard allocation locally using local and protocol information.
  - A unique identity will be created for each plot, and this identity is the one considered to
    allocate the power from the plot into a specific shard.
  - Without a priori knowledge of the list of validators at a given epoch, we can't use a weighted
    allocation. We can have knowledge about the total storage of the system from the beacon chain,
    but there is no way to have knowledge about all the existing plots so they can be order
    increasingly and perform a weighted assignment. Thus, the protocol considers a `MAX_PLOT_SIZE`
    allowed for plots, and this is the power considered for all plots in the allocation (as we will
    see below in the analysis, this considers the worst case scenario in terms of security, and the
    impact in terms of shard load balancing should be negligible if the number of plots in the
    system is large).

- Only leaf shards are considered for the membership allocation. When a farmer is assigned to a leaf
  shard it will need to also sync its parent and (of course) the beacon chain.

  - By only performing membership allocation in the beacon chain we implicitly also load-balance and
    distribute efficiently the power in intermediate shards simplifying the implementation of the
    allocation mechanism.
  - Thus, when plotting farmers will have to decide if they want to allocate their storage into the
    smallest possible number of plots according to `MAX_PLOT_SIZE` to minimise the number of shards
    they are assigned to (and that, consequently, they need to sync with), or if they want to have a
    lot of smaller plots with the corresponding overhead that this imposes.

## Protocol parameters

- These are the protocol parameters considered for the core allocation mechanism:

  - `NUM_SHARDS`: The number of leaf shards in the system.
  - `MAX_PLOT_SIZE`: The maximum size of a plot in bytes.
  - `total_storage`: The total storage capacity of the system in bytes. We can infer this metric
    from the beacon chain
  - `plot_j`: Unique identity for a specific plot (e.g. ID `j)
  - `MEMBERSHIP_RESHUFFLE_INTERVAL`: The interval between membership reshuffles in slots.
  - `NEW_MEMBERSHIP_WARMUP_INTERVAL`: The interval between the membership reshuffle and the new
    membership coming to effect in slots.

## Protocol operation and analysis

The membership allocation protocol operates as follows:

- Every `MEMBERSHIP_RESHUFFLE_INTERVAL` slots, the membership allocation protocol is triggered.
- The target storage per shard is first computed as
  `target_storage_per_shard = total_storage / NUM_SHARDS`.
- The specific shard that each plot for a farmer is allocated to can be computed locally leveraging
  a VRF of the plot identity, feeding the randomness from the PoT chain at the
  `MEMBERSHIP_RESHUFFLE_INTERVAL` epoch. This allocation can be computed using any other method (we
  don't necessarily need to use a VRF) as long as is uniform, randomness and can't be easily biased.

```
shard_id = VRF(plot_identity, epoch_randomness) % NUM_SHARDS
```

- This approach should allocate approximately the following number of plots per shard:
  `plots_per_shard = floor(target_storage_per_shard / MAX_PLOT_SIZE)`.

- We want the protocol to be secure against static adversaries, i.e. from an attacker trying to
  create plots in a way that allows them to allocate a majority of power in a single shard taking
  control of it. We can compute the probability of a farmer being able to attack using a binomial
  distribution of the number of shards required to get control of a random shard:

> TODO: Use a more reader friendly notation for this in the next round of edits.

$$
P(X_k \ge k_\alpha) = \sum_{j=k_\alpha}^{K_M} \binom{K_M}{j} \left(\frac{1}{N}\right)^j \left(1 - \frac{1}{N}\right)^{K_M - j}
$$

where:

- ka = is the number of plots that the attacker needs to get control of a shard. For a longest-chain
  protocol like ours 51%. This can be computed as
  `ka = (percentage_storage * target_storage_shard) / MAX_PLOT_SIZE`
- KM = total number of plots owned by the farmer = `farmer_storage / MAX_PLOT_SIZE`
- N = number of shards

- From this result, and considering the target security guarantees and technical restrictions for
  the protocol, we should assign the optimal reshuffling interval in a way that the probability of a
  farmer being able to take control of a shard in an interval is negligible. Thus, considering that
  the probability of attack is a decreasing function of the reshuffling interval, and that the cost
  of re-shuffling increases with the inverse of the reshuffling interval, we need to ensure that the
  optimal interval minimises the probability of an attack, and is manageable for farmers considering
  the cost of reshuffling.

  - The main cost of reshuffling is the cost of syncing from scratch with the new shard (or shards)
    that the farmer has been assigned to. Hence, the `NEW_MEMBERSHIP_WARMUP_INTERVAL` should be
    large enough to allow for all farmers to prepare for the new allocation.
  - While `NEW_MEMBERSHIP_WARMUP_INTERVAL` is part of the `MEMBERSHIP_RESHUFFLE_INTERVAL`, and its
    value does not change the probability of attack of farmers trying to allocate a great amount of
    power in a single shard, it opens a new surface attack where the randomness used for plot
    allocation has already been released, and an attacker may try to predict the assignment of
    existing plots to DDoS or take control of certain plots from a farmer (and thus pull off an
    attack). This attack, along with others that may involve bribery and collusion are analysed in
    the section below as part of the fully adaptive adversary.

> The script at `./farmer_allocation.py` implements a simple model that allows you to adjust
> different protocol parameters and network scenarios to evaluate the security guarantees of the
> system and even estimate the economic cost required to pull off an attack with high probability.

## Protocol Extensions

Apart from protecting against static adversaries, the protocol should be able to also protect
against fully adaptive adversaries. A fully adaptive adversary can:

- Chose what nodes to compromise and release making their attack vectors fluid and responsive.
- Access real-time information: They have comprehensive, real-time knowledge of the entire network's
  state, including current shard assignments, observed network activity, and the precise outcomes of
  random processes (like VRFs) as soon as they're publicly known.
- Their actions are immediate and informed. If they detect a vulnerable set of honest nodes assigned
  to a specific shard, they can instantly corrupt enough of them to gain a majority. This allows
  them to maximize their impact by targeting weaknesses as they emerge.

To prevent this type adversaries, a dynamic allocation within an interval needs to be introduced
(similar to the one proposed in the Free2Shard paper) to protect against these sophisticated
attacks. While this mechanism doesn't need to be released immediately in the network, we can
introduce this protocol extension as soon as we want (or need) to protect against fully adaptive
attackers:

- Protocol Parameters

  - `BALANCING_INTERVAL`: This is a short time interval (in slots) at which the dynamic load
    balancing protocol runs. It's much shorter than `MEMBERSHIP_RESHUFFLE_INTERVAL`, enabling rapid
    responses.
  - `BALANCING_THRESHOLD`: A specific deviation threshold. If a shard's verifiable activity or
    "difficulty" goes beyond this threshold (either too high or too low), it's considered
    "misrepresented" or "imbalanced," triggering a re-balancing action.
  - `BALANCING_WARMUP_INTERVAL`: This is the time (in slots) between when a partial re-allocation is
    decided during load balancing and when those new assignments actually become effective. It's
    similar to `NEW_MEMBERSHIP_WARMUP_INTERVAL` but applies only to the re-balanced farmers.

- Protocol extension operation

1.  Verifiable Shard Health Monitoring & Activity Assessment: We won't rely on self-reported
    difficulty from shards, as that can be faked by an adaptive adversary. Instead, the beacon chain
    (or a randomly selected group of honest validators/auditors) will verifiably monitor each leaf
    shard's health and activity.

- Beacon Chain Observation: The beacon chain continuously tracks key metrics from each shard's
  submitted block headers:
  - Block production rate through the submission of blocks from the lower levels of the hierarchy.
  - Perceived difficulty that can be inspected in the header of the submitted blocks.
  - Other activity metrics like segment commitment (and validation)?
  - What we want out of this is an objective measure of the power in the shard, their liveness and
    security. Let's consider the "difficulty" as the current "proxy" measure that we currently have
    for this.

1.  Misrepresentation Detection:

- The verifiably measured "difficulty" or activity of each shard is compared against its ideal
  target_storage_per_shard.
- If a shard's metric deviates beyond the BALANCING_THRESHOLD, it's marked as "over-represented"
  (too much active power) or "under-represented" (too little active power, potentially compromised).

1.  Targeted Re-allocation:

- For each "misrepresented" shard, a subset of its current farmers is selected for re-allocation.
  The number of farmers is proportional to how much the shard is off-balance.
- These selected farmers (identified by their plot IDs) are then randomly assigned to new shards
  using the same VRF mechanism: new_shard_id = VRF(plot_identity, current_epoch_randomness) %
  NUM_SHARDS
  - Note: When performing this allocation we should consider if the farmer previous allocation is
    invalidated or if this is an additional allocation that is assigned randomly on top of the
    existing one
- This ensures the re-allocation is random, preventing the adversary from predicting new assignments
  for their re-allocated plots.

1.  Warmup Period for Balancing (BALANCING_WARMUP_INTERVAL):

- Once the new assignments for re-allocated farmers are set, a BALANCING_WARMUP_INTERVAL begins.
- During this time, these farmers must synchronize with their new shards and stop participating in
  their old ones.
- This interval must be long enough for syncing, but short enough to minimize the window for
  adaptive attacks specifically targeting these transitioning farmers.
