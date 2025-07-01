---
title: Expiring Sharded Subspace Plots and improving model script
date: 2025-06-30
draft: false
description:
  Commiting sectors to a range of history sizes to maintain Subspace's plot expiration logic
tags: [status-update, consensus]
authors: [adlrocha]
---

As mentioned on last week's status update, one of the key pieces that I was to have the detailed
operation of plot membership allocation was the impact of sector expiration on the protocol. By
having a unique plot identifier, we are able to uniquely link sectors to plots, but these sectors
need to expire in a way that does not require farmers to re-plot while archiving the most recent
history. Fortunately, we can leverage the current expiration mechanism of the Subspace
protocol, and build a layer on top of it to adapt it to the sharded version while maintaining the
original guarantees in terms of plot expiration, sector re-plotting, and history archiving.

<!--more-->

## Sharded Subspace Plot Expiration Protocol

### Overview

Before I start with the description of the protocol, let me remind you what are the core properties
that we wanted the protocol to have:

1.  **Unique Plot IDs:** `plot_id` is unique and stable. This is key for the protocol, as this ID
    will be used to allocate space into shards.
2.  **No Bias through `history_size`:** Farmers can't game shard allocation by choosing among any
    `history_size`, or by creating sectors with the same ID committed to different history size.
    This prevents an attack where the same plot ID could be used to create sectors with different
    `history_size` values, which would allow farmers to manipulate shard allocation by assigning to
    a plot the most convenient sector at each time.
3.  **Load-Balanced Archiving (New History):** As new history arrives, it gets stored across
    farmers/plots without large re-plotting events.
4.  **Detect Expiration and Validity:** Plots, sectors and blocks include all the information needed
    to verify their validity, plot assignment to a shard, sector belonging to the plot, and
    expiration.
5.  **No Excessive Re-plotting:** Keep re-plotting burden similar to original Subspace.

### Committing sectors to history ranges

The core idea for sector expiration is the following: instead of letting farmers commit their
sectors to any history size, they are bound to commit them to a range of history sizes determined by
the plot the sector is linked to. This way, we limit the range of history sizes that a sector can be
committed to, and hence the parallel sectors with the same `plot_id` that and different committed
history sizes that can be created. This is implemented as a "self-evolving plot commitment range"
that grows as the blockchain grows on top of the existing mechanism for sector expiration. Thus, we
maintain all the properties of the original Subspace protocol, while adapting it to the needs of the
sharded version. This allows us to also learn an apply the heuristics from the live Subspace
network.

The high-level operation of the protocol is as follows:

- When creating a new plot, farmers need to choose a random offset (essentially a nonce) for their
  plot. This offset determines the specific ID of the plot and will influence the history range that
  the plot's sectors can commit to. As a reminder, plots identities are inferred from the public key
  hash of the farmer, and the random offset they choose:

```rust
plot_id = hash(public_key_hash || offset)
```

- When creating new sectors for a plot, farmers will choose any valid history size within the plot's
  current range. Piece selection is based on the sector's specific history size, and expiration
  follows the same logic as the original Subspace protocol. Finally, the sector ID is derived from
  the plot ID, sector index, and committed history size:

```rust
sector_id = keyed_hash(plot_id, sector_index || committed_history_size)
```

- We determine a protocol parameter `BASE_WINDOW_SIZE` that determines the size of the `BASE_RANGE`.
  This `BASE_RANGE` determines the baseline history range for all plots. This `BASE_RANGE`
  determines the size of history sizes that plots can commit to initially. The plot offset modifies
  the `BASE_RANGE` to create a unique range for each plot ID, and as the history of the chain grows,
  this is expanded accordingly so it can fit new history sizes. The following image belongs to a
  [simple visualisation tool](./2025-06-30-sector-expiration/range-evolution-visual.html) that an
  LLM has built for me to visualise how the base range and effective range of a plot behaves as the
  history of the chain grows:

<p align="center">
<img alt="Range evolution visualisation" src="range_visualisation.png">
</p>

- Sectors expire as they are used to in current Subspace protocol, so even if the effective range
  for plots grow, sectors committed to old history sizes will be alreaedy expired (or close to
  expire) still limiting the number of history sizes that sectors can commit to at a given time.
  This is key to ensure that farmers do not need to re-plot their sectors as the history grows, and
  that they can archive the most recent history without having to re-plot, maintaining all the nice
  properties for sector expiration of the original Subspace protocol.

### Putting all pieces together

With this, we have all that we needed to finally implement farming in shards:

- Plot IDs are unique and stable, allowing us to allocate space into shards.
- Sectors are committed to a range of history sizes determined by the plot ID, preventing farmers
  from gaming shard allocation.
- Piece selection, sector expiration stay unchanged.
- Farming stays mostly unchanged, apart from the additional information required to verify that
  the chunk for the piece in the solution range belongs to a sector bound to a plot currently
  assigned to the shard.

This is how the verification logic for a solution in a shard would look like:

- Standard Subspace Verification (Unchanged): First, perform all existing Subspace checks

```rust
// Verify piece offset is valid
if piece_offset >= max_pieces_in_sector {
    return Err("Invalid piece offset");
}

// Check sector expiration
if sector_is_expired(sector_id, current_history_size) {
    return Err("Sector has expired");
}

// Verify the piece is part of blockchain history
if !verify_piece_inclusion(piece, segment_root, proof) {
    return Err("Invalid piece proof");
}
```

- Plot Identity Verification: Verify that the solution comes from a valid plot:

```rust
// Step 2.1: Compute expected plot ID from farmer's public key and offset
expected_plot_id = hash(farmer_public_key || plot_offset)

// Step 2.2: Verify the sector was created with this plot ID
// The sector_id should incorporate the plot_id
if !sector_belongs_to_plot(sector_id, expected_plot_id) {
    return Err("Sector doesn't belong to claimed plot");
}
```

- Range Validation: Ensure the sector's committed history size is within the plot's valid range:

```rust
// Step 3.1: Calculate the plot's base range from its offset
window_level = plot_offset / OFFSETS_PER_WINDOW
window_size = BASE_WINDOW_SIZE * (2 ^ window_level)
offset_in_window = plot_offset % OFFSETS_PER_WINDOW

min_history = (window_size * offset_in_window / OFFSETS_PER_WINDOW) - GENESIS_OFFSET
max_history = (window_size * (offset_in_window + 1) / OFFSETS_PER_WINDOW) - GENESIS_OFFSET

// Step 3.2: Calculate effective range (extends with blockchain growth)
effective_max_history = max(max_history, current_history_size)

// Step 3.3: Verify sector's history size is within range
if sector_history_size < min_history || sector_history_size > effective_max_history {
    return Err("Sector history size outside plot's valid range");
}
```

- Shard Assignment Verification: Confirm the plot is assigned to the current shard:

```rust
// Step 4.1: Determine which shard this plot belongs to
// Use the plot ID and base range for stable assignment
expected_shard_id = VRF(plot_id || min_history) % NUM_SHARDS

// Step 4.2: Verify this matches the shard for the block being verified
if expected_shard_id != current_shard_id {
    return Err("Plot is not assigned to this shard");
}
```

To perform sharded verification, clients need:

- From the Solution

  - `farmer_public_key`: The farmer's identity
  - `plot_offset`: The offset chosen when creating the plot
  - `sector_index`: Which sector within the plot
  - `sector_history_size`: The history size this sector committed to
  - `piece_offset`: Which piece within the sector
  - Standard Subspace proofs (piece inclusion, etc.)

- From the Block Header and the network
  - `current_history_size`: Current blockchain history size
  - `current_shard_id`: Which shard is performing verification
  - Protocol constants (BASE_WINDOW_SIZE, NUM_SHARDS, etc.)

### A script to tinker with and next steps!

This week I've also been spending some time implementing all the formal models from the last few
updates into a Python script that allow us to tinker with the protocol and see how it impacts its
security and correctness. So far the results are promising, but it still needs a few iterations to
double-check that the model makes sense and that the protocol is sound. I am also working on try to
extensively document the script so others can pick it up and play with it and/or point out potential
mistakes.

<p align="center">
<img alt="Screenshot from protocol modelling script" src="script_screenshot.png">
</p>

With this in mind, this week I will be focusing on:

- Mapping all the sector expiration and history range logic into pseudocode so Nazar can start
  implementing the validation logic that will in itself start enabling adapting plotting and farming
  to shards.
- Exploring the implementation of a simple simulator that allows us to simulate the protocol and its
  properties, so we can start reasoning about different parameters and its specific sub-protocols
  more concretely than the current Python script.

Let's see how far we get this week! I am excited to see how this will evolve and how we can start
building the foundation for a more robust and scalable protocol. As always, reach out if you have
any questions, suggestions, or feedback. Until next week!
