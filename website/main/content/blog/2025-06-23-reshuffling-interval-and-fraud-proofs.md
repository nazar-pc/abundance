---
title: Reshuffling interval and living without fraud proofs
date: 2025-06-23
draft: false
description: Can we implement a protocol that does not require fraud proofs?
tags: [status-update, consensus]
authors: [adlrocha]
---

{{< katex >}}

Last week I shared a model that can help us reason about the security of shards assuming an honest
majority in the beacon chain. The model evaluates what are the trade-offs in terms of the number of
shards, the number of farmers per shard, and the proportion of malicious farmers in the system. But
if you recall from the overall design of the system, shards are periodically submitting segments and
blocks to the upper layers of the hierarchy and to the beacon chain. We need to verify that these
are valid, available, and correctly encoded before they are included in super segments and the
global history of the system. Can we do so without relying on fraud proofs? This has been one of my
focuses for the week, let's jump right into it.

<!--more-->

## Farmer reshuffling to prevent fraud proofs

Let me refresh your memories by describing how we are planning to work around requiring fraud
proofs. The core idea here is that by leveraging the reshuffling of farmers (or actually plots)
across the different shards, and the fact that when farmers are assigned to a new shard they need to
sync and verify the latest history of that shard, we can ensure that any fraudulent segment will be
detected by honest farmers.

- If a malicious farmer in a shard creates an invalid block or segment (invalid transaction, state
  transition, incorrect encoding, etc.), _eventually_ an honest farmer will be assigned to that
  shard during a reshuffle. This honest farmer will download and verify the latest shard's history.
  Upon encountering the invalid block or segment, they will detect the fraud and submit it to its
  parent and the beacon chain. Intermediate shards and the beacon chain do not immediately accept
  new blocks being submitted by child shards, they wait for one or more reshuffling intervals to
  ensure that the data has been verified by honest farmers. In thew next section I will share the
  model to determine the optimal values depending on the desired security level.
- When the honest farmer detects fraud, they will refuse to build on top of the fraudulent chain. If
  the shard has a sufficient number of honest farmers that detect this, they will collectively build
  a valid fork, effectively causing a re-org that prunes the fraudulent history.
- This mechanism also addresses data availability directly. If a block or a segment is unavailable,
  the syncing farmer cannot download it and thus cannot reconstruct the state. This inability to
  sync would trigger the same "re-org" or "rejection" behavior, preventing the malicious chain from
  progressing as new farmers will only be able to grow the chain from the latest valid state. The
  missing block or segment would have to be re-created. The assumption is that if data is not
  available, it effectively means the chain cannot be validly extended by honest participants and
  the chain needs to be recovered from the last known state.

Skimming through the literature around sharded blockchains and fraud proofs uncovered that this
approach is often referred to as "_periodic reshuffling with full-state downloads"_ or
_state-sync-based fraud detection_ in other protocols. This looks promising, but we need to model it
to understand the implications in terms of security and the trade-offs involved (especially
regarding the reshuffling interval).

## Modeling the Reshuffling Interval (R)

Let's define some parameters and then formalise the security of the mechanism.

### Key Parameters:

- \\(N\\): Total number of farmers (or plots) in the system.
- \\(S\\): Total number of shards.
- \\(n=N/S\\): Average number of farmers per shard.
- \\(p\\): Proportion of malicious farmers in the entire system.
- \\(f\\): Minimum proportion of honest farmers required within a shard to detect and force a
  re-org. This is typically \\(>0.5\\) (e.g., \\(f=0.51\\)) to outvote or out-compute the malicious
  actors.
- \\(T_B\\): Average block time of the beacon chain.
- \\(T_S\\): Average block time of a shard.
- \\(L\\): Number of blocks in a reshuffling interval (i.e., reshuffling happens every \\(L\\)
  blocks on the beacon chain).
- \\(R=L \cdot T_B\\): Duration of a reshuffling interval in real time.
- \\(\\alpha\_{fraud}\\): Acceptable probability that a fraudulent segment remains undetected after
  a certain number of reshuffles. This is your desired "security level" against undetected fraud.

## Modeling the Detection Probability:

### Probability of a Malicious Shard:

Using the Chernoff bound from my previous update, the probability that a specific shard has \\( \ge
f \cdot n \\) malicious farmers is:

$$
P(\text{Shard } k \text{ is insecure}) = P(M_k/n_k \ge f)
\le e^{-n(f-p)^2/(3p)}
$$

(assuming \\(f>p\\)).

Let this be \\(P\_{\text{insecure_shard}}\\). This is the probability that a shard is immediately
vulnerable upon formation.

### Probability of an Honest Farmer Being Assigned to a Shard:

In each reshuffling interval, a farmer is randomly assigned to a shard. The probability that a
specific honest farmer is assigned to a specific shard is \\(1/S\\).

More generally, the probability that a newly assigned farmer to a shard is honest is \\(1-p\\).

### Probability of Detecting Fraud in One Interval:

Consider a shard where fraud has occurred. We need an honest farmer to be assigned to this shard.

Let \\(k\\) be the number of farmers assigned to a shard during one reshuffling. For simplicity,
let's assume \\(k \approx n\\) (the average).

The probability that at least one of the \\(n\\) newly assigned farmers to a specific shard is
honest is:
$$P(\text{at least one honest farmer in new assignment}) = 1 - P(\text{all new assignments are malicious})$$
$$P(\text{at least one honest in new assignment}) = 1 - p^n$$

This is an oversimplification, as it assumes all \\(n\\) farmers are new. In reality, farmers are
shuffled amongst existing shards, and some farmers might remain in the same shard, or might be
replaced by other farmers. A more accurate model considers that a random subset of \\(n\\) farmers
are drawn from the \\(N\\) total farmers and assigned to this specific shard, but our case is closer
to the scenario where all farmers are new.

Let's refine: For a specific shard that has committed fraud, what's the probability that none of the
\\(n\\) farmers assigned to it in the next reshuffling interval are honest? This is \\(p^n\\) if
assignments are done with replacement from the global pool (which is a reasonable approximation for
large \\(N\\)).

So, \\(P(\text{fraud detected by new assignment in 1 interval}) = 1 - p^n\\).

This assumes that if an honest farmer is assigned, they will detect the fraud and initiate a re-org.

### Probability of Fraud Remaining Undetected After k Reshuffles:

If a fraudulent segment or a block is submitted to the upper layers of the hierarchy, it potentially
remains undetected until an honest farmer is assigned to that shard and syncs.

The probability that fraud in a specific shard remains undetected for \\(k\\) consecutive reshuffles
is \\((p^n)^k\\).

So, \\(P(\text{fraud detected by interval } k) = 1 - (p^n)^k\\).

### Overall Security Bound (\\(\alpha\_{fraud}\\)):

We want the probability that any fraudulent segment or block remains undetected for \\(k\\)
intervals to be very low. \\(P(\text{any fraud undetected for } k \text{ intervals}) \le S \cdot
(p^n)^k\\) (Union Bound over shards)

Setting this to \\( \alpha*{fraud} \\): \\( S \cdot (p^n)^k < \alpha*{fraud} \\)

This equation allows us to reason about the trade-offs (that actually match the intuition we had):

- Increasing \\(k\\) (more reshuffles): Decreases the probability of undetected fraud, but increases
  the time a segment might be considered "tentatively final" before true probabilistic finality.
- Decreasing \\(p\\) (fewer malicious farmers): Drastically reduces the probability, but implies a
  stronger honest majority assumption.
- Increasing \\(n\\) (fewer shards/more farmers per shard): Drastically reduces the probability, but
  impacts scalability.
- Decreasing \\(S\\) (fewer shards): Reduces the number of opportunities for fraud across the
  system.

### Solving for \\(k\\) (number of reshuffles until detection):

Given \\(S, p, n, \alpha*{fraud}\\): \\(k > \frac{\log(\alpha*{fraud}/S)}{\log(p^n)}\\)

This \\(k\\) gives use the number of reshuffling intervals we might need to "wait" before a block or
a segment can be considered probabilistically secure against undetected fraud. If a block or segment
submitted to the beacon chain and eventually included in a SuperSegment at beacon height \\(H\\),
and \\(k\\) reshuffles happen, the "true" finality could be argued to be at beacon height \\(H+k
\cdot L\\).

With this, we can determine the optimal reshuffling interval based on the desired security level
(\\(\alpha\_{fraud}\\)) and the parameters of the system.

## Farming in shards

Another piece of the puzzle that I left behind from my previous update is how farmers are chosen to
propose the next block in the shard they've been allocated to, and how can this allocation be
verified by the rest of the network. Last week, we introduced the concept of a _PlotID_ used to
uniquely identify a plot (i.e. a batch of storage sectors), and used to determine the allocation of
storage in the system.

Farming in shards work in the same way as it currently works in the Subspace protocol, with the main
difference that now the winning farmer will have to also present along with the block a proof that
one of its plots belongs to the shard and hence is entitled to propose a new block in that shard.

If we look at the data structure from below, we can see that by including the `plot_id` that
determines the allocation of a farmer's storage to the shard and some additional information, we can
determine if the farmer is entitle to propose a block in the shard.

```rust
// -- The shard ID is explicitly shared in the block header.
struct Solution {
    public_key_hash:          32 bytes
    sector_index:         2 bytes
    history_size:         8 bytes
    piece_offset:         2 bytes
    record_commitment:   48 bytes
    record_witness:      48 bytes
    chunk:               32 bytes
    chunk_witness:       48 bytes
    proof_of_space:     160 bytes
    // --- NEW FIELD FOR MEMBERSHIP AND PLOT UNIQUENESS PROOF ---
    plot_id:             32 bytes, // Unique identifier for the plot
}

    // Additional information that we may need to include as part of the `Solution` struct
    // if we make the configurable instead of protocol parameters (we can keep them out of the struct
    // but consider them available for verification for the sake of this discussion.)
    max_plot_size:       64 bytes, // Maximum size of the plot.

    // These two parameters may be simplified into a single one depending on how we end up implementing re-plotting and
    // the expiration of sectors as it will be described below.
    min_history_size:    8 bytes, // Minimum history size the sectors in the plot are allowed to commit to.
    max_history_size:    8 bytes, // Maximum history size the sectors in the plot are allowed to commit to.

    // This parameters determines that randomness used in the last reshuffling interval to determine the membership allocation,
    // and will be required to determine that the proposing farmer is currently allocated to the shard.
    membership_allocation_randomness_seed
```

At a really high-level these are the steps required to verify that a block includes the right
solution, and that the farmer plot is allocated to the shard:

- **Standard Subspace PoSt Verification**: It works in the same as it currently works in Subspace.

  - Verify `public_key` is not in block list.
  - Verify consensus log (`solution_range`, `global_randomness`).
  - Verify PoT items.
  - Compute `global_challenge = hash(global_randomness || slot_number)`.
  - Verify `current_chain_history_size > winning_piece_index / NUM_PIECES`. (This is key for
    `history_size` validity).
  - Verify `piece_offset <= max_pieces_in_sector`.
  - Derive `public_key_hash = hash(public_key)` (already in current process).
  - Re-derive the `sector_id`

- **Verify Implicit `plot_id` Consistency and Uniqueness Constraints:**

  - Re-derive `declared_plot_id`:
    `declared_plot_id = keyed_hash(solution.public_key_hash, solution.max_plot_size || solution.min_history_size || solution.max_history_size)`
    (Note: `public_key_hash` is computed from `block.public_key`).
  - Verify `history_size` falls within plot's declared window: Check that
    `solution.history_size >= solution.min_history_size` AND
    `solution.history_size <= solution.max_history_size`. _This is the core check preventing
    "double-committing sectors" to different history sizes for allocation bias._ If a farmer tries
    to submit a `sector_id` that uses a `history_size` outside the _declared_
    `[min_history_size, max_history_size]` window for that `plot_id`, the block is invalid.
  - Derive `constrained_sector_id`:
    `constrained_sector_id = keyed_hash(declared_plot_id, solution.sector_index || solution.history_size)`
    _This is the critical new `sector_id` that incorporates the plot's history window constraint._
  - Now, use `constrained_sector_id` for all subsequent verification steps that currently use the
    `public_key_hash` derived `sector_id`:
    - Replace `sector_id` in `sector_expiration_check_history_size`, `expiration_history_size`,
      `sector_slot_challenge`, `s_bucket_audit_index`, `evaluation_seed` with
      `constrained_sector_id`.
    - This means the farmer's proof must ultimately derive from a `sector_id` that respects the
      `plot_id`'s declared history boundaries.

- **Verify Shard Assignment (incorporating `plot_id`):**

  - Retrieve the `randomness_seed` from the beacon chain state root at
    `solution.shard_assignment_seed_beacon_height` and
    `solution.shard_assignment_seed_beacon_block_hash`. (This assumes the verifier has access to
    canonical beacon chain history).
  - Compute `expected_shard_id = VRF_verify(randomness_seed || declared_plot_id) % S`.
  - Check that `expected_shard_id == ShardBlock.header.shard_id`. This proves the farmer was
    legitimately assigned to propose for this shard based on their _declared plot_.

- **Complete Existing Subspace PoSt Verification (using `constrained_sector_id`):**

  - Verify `sector_id` expiration (now using `constrained_sector_id`).
  - Re-derive `sector_slot_challenge = global_challenge XOR constrained_sector_id`.
  - Re-derive `s_bucket_audit_index = sector_slot_challenge mod NUM_S_BUCKETS`.
  - Re-derive `evaluation_seed = hash(constrained_sector_id || piece_offset)`.
  - Verify `proof_of_space`.
  - Ensure `chunk` satisfies challenge criteria (using `sector_slot_challenge` from
    `constrained_sector_id`).
  - Verify `chunk_witness`.
  - Re-derive `piece_index` (this uses `history_size`, which is checked against plot bounds).
  - Retrieve `segment_commitment` (needs to match `history_size`).
  - Verify `record_witness`.
  - Verify `farmer_signature` on `chunk`.
  - Verify signature on block content.
  - Check for equivocation.

## What's next?

The farming protocol presented above assumed a really specific mechanism for sector expiration and
re-plotting. Unfortunately, after discussing it with Nazar we realised that my re-plotting proposal
had some wholes, so while the high-level of the verification stands, expect some minor changes in
the information included in the header and the verification process. This will mainly be related
with the history size window of the plot. I don't expect any big refactor to be needed here, but
stay tuned!

With this in mind, my two main focus for this week are on:

- Detailing the protocol for sector expiration in plots while maintaining the commitment of plots to
  a specific history size through their plot ID.
- Updating the python script that I wrote a few weeks ago that allows us to model and tweak the
  parameters of the protocol with the new model of the past two weeks merging all together to try to
  surface the optimal protocol parameters and what that entails to the security of the system.

And that's all for this week, looking forward to share my progress next week (hopefully with a big
batch of good results).
