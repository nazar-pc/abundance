# Shard segments submission to the beacon chain

## Committing segments to the global history in the beacon chain

The goal of this discussion is to surface the core data structures and mechanics of the process of
committing segments to the global history in the beacon chain.

1. Each child shard creates segments independently (in the same way that is currently implemented).
   Segments are assigned and increasing sequence number, `local_index` that determines the order in
   which they were created in in the shard. These segments that haven't been committed to the global
   history in the beacon chain are `UnverifiedSegment`s and include their own
   `UnverifiedSegmentHeader` (which matches the current `Segment` and `SegmentHeader` data
   structures, respectively).

```rust
pub struct UnverifiedSegmentHeader {
    /// Segment Local Index
    pub local_index: Unaligned<SegmentIndex>,
    /// Root of roots of all records in a segment.
    pub segment_root: SegmentRoot,
    /// Hash of the segment header of the previous segment
    pub prev_segment_header_hash: Blake3Hash,
    /// Last archived block
    pub last_archived_block: LastArchivedBlock,
}

pub struct RecordedHistorySegment([Record; Self::NUM_RAW_RECORDS]);
```

2. As soon as a new segment has been created in a shard, information about their local segment
   index, segment root (and implicitly their shard ID) is included in the next block information and
   submitted to the parent shard as part of `own_segment_roots` of `LeafShardBlocksInfo`.

> NOTE: Instead of just sending segment roots in `LeafShardBlockInfo` as it is currently the case in
> the code, we also include the local segment index to help with verification and linking of
> segments. That way, we can implicitly reference segments without having to provide their segment
> root through their shard ID and local segment index. This will be necessary to minimise the flow
> of information when reporting the finality of segments as described below.

```rust
pub struct LeafShardBlockInfo<'a> {
    /// Block header that corresponds to an intermediate shard
    pub header: LeafShardBlockHeader<'a>,
    /// Segment roots proof if there are segment roots in the corresponding block
    pub segment_roots_proof: Option<&'a [u8; 32]>,
    /// Information about segments produced by this shard
    pub own_segment_info: &'a [(SegmentRoot, SegmentIndex)],
}
```

3. If the parent of the shard is an intermediate shard, it will pull the segment information of its
   child and include it as part of the `child_segment_roots` inside the next
   `IntermediateShardBlockInfo` to be submitted to the parent chain (i.e. the beacon chain), along
   with any of the segments created in that block in the shard, which are included in the
   `own_segments` field of the data structure. `ShardBlockInfo` data structures include a
   `segment_roots_proof` that proves that indeed the segments where included in the block body of
   the corresponding block.

   ```rust
   pub struct IntermediateShardBlockInfo<'a> {
    /// Block header that corresponds to an intermediate shard
    pub header: IntermediateShardBlockHeader<'a>,
    /// Segment roots proof if there are segment roots in the corresponding block
    pub segment_roots_proof: Option<&'a [u8; 32]>,
    /// Segment roots produced by this shard
    pub own_segment_info: &'a [(SegmentRoot, SegmentIndex)],
    /// Segment info produced by child shard
    pub child_segment_info: &'a [(ShardId, SegmentRoot, SegmentIndex)],

   }
   ```

4. When parent chains (both intermediate shards and the beacon chain), receive a
   `LeafShardBlockInfo` and `IntermediateShardBlockInfo`, respectively, farmers verify that the
   information included is valid. Mainly, that the `ShardBlockHeader` being submitted in the
   `ShardBlockInfo` is valid, that the `segment_roots_proof` is correct, and that the
   `own_segment_info` and `child_segment_info` are part of the segments created in the shard. If
   this is the case, the parent shard will include this information in its own block in the
   `leaf_shard_blocks` and `intermediate_shard_blocks` fields, respectively.

```rust
pub struct IntermediateShardBlockBody {
    /// Segment info produced by this shard
    pub own_segment_info: [(SegmentRoot, SegmentIndex)],
    /// Leaf shard blocks
    pub leaf_shard_blocks: [LeafShardBlocksInfo],
    /// User transactions
    pub transactions: Transactions,
}

pub struct BeaconChainBlockBody {
    /// Segment roots produced by this shard
    pub own_segment_info: [(SegmentRoot, SegmentIndex)],
    /// Intermediate shard blocks
    pub intermediate_shard_blocks: [IntermediateShardBlocksInfo],
    /// Proof of time checkpoints from after future proof of time of the parent block to current
    /// block's future proof of time (inclusive)
    pub pot_checkpoints: [PotCheckpoints],
}
```

5. In this way, new shard segments from leaf and intermediate shards are immediately communicated to
   the beacon chain. However, segments are not included into the global history and added to a super
   segment until the segments can be considered final and have a low probability of re-org. To
   identify when a segment can be considered as final, the following steps are taken:

   - When farmers in an intermediate shard receive a `LeafShardBlockInfo`, they keep track locally
     of the segment information and the block from the leaf shard where the segment was included,
     determined by the `header` field of `LeafShardBlockInfo`. For each recent block submitted,
     farmers compute the probability of re-org for the block. The probability of re-org can be
     computed in the following way:

   ```rust
   Prob_reorg(depth_of_reorg) = (2 * time_first_block_propagation / average_block_time) ^ depth_of_reorg
   ```

   - The probability of re-org is inferred from the probability of a block being orphaned due to
     network latency which is approximately `2 * time_first_block_propagation / avg_block_time`,
     being `2*time_first_propagation` the vulnerable window of time where orphan blocks can happen.
     These values can be set as constants in the implementation according to some protocol
     heuristics or be dynamically computed in every farmer according to their view of the chain
     health.
   - Thus, for each recent block, farmers can compute the probability of re-org according to their
     depth in the chain. For example, if the current heaviest chain is at height 100, the
     probability of re-org for the head of the chain is computed for depth 0, the previous block at
     depth 1, and so on. When the probability of a re-org for certain depth of a recent block is
     below the `FINALITY_PROBABILITY_THRESHOLD` computed as `1-prob_reorg(depth_of_reorg)`, then the
     segment roots of that block are considered final and can be confirmed to the beacon chain
     inside a `final_child_segments` field of the `IntermediateShardBlockInfo`.

   > Note: This requires changes into the current `IntermediateShardBlockInfo` data structure to
   > include this field. This is a requirement to make segments from leaf shards available as soon
   > as possible in the beacon chain, which will be useful for segment availability checks and
   > verification. If we can't afford adding this additional field we may need to just delay the
   > leaf shard segment commitment to the beacon chain from the intermediate shard until it can be
   > considered final by the intermediate shard (that way the beacon chain can include it to the
   > global history as soon as it receives it, otherwise it can't be sure that the re-org
   > vulnerability window for the leaf shard has passed).

   ```rust
   /// Modified IntermediateShardBlockInfo to include final child segments
   pub struct IntermediateShardBlockInfo<'a> {
    /// Block header that corresponds to an intermediate shard
    pub header: IntermediateShardBlockHeader<'a>,
    /// Segment roots proof if there are segment roots in the corresponding block
    pub segment_roots_proof: Option<&'a [u8; 32]>,
    /// Segment roots produced by this shard
    pub own_segment_info: &'a [(SegmentRoot, SegmentIndex)],
    /// Segment info produced by child shard
    pub child_segment_info: &'a [(ShardId, SegmentRoot, SegmentIndex)],
    /// Final child segments that can be considered final and included in the global history
    pub final_child_segments: &'a [(ShardId, SegmentIndex)],
   }
   ```

6. The beacon chain tracks the specific block heights from the intermediate shard at which
   `child_segment_info` and `final_child_segments` were submitted, and will follow the same approach
   used in intermediate shards to identify when a block can be considered final. Thus:

   - Leaf shard segments will be included to the global history in the beacon chain once its parent
     shard has submitted a `segment_info` and its subsequent `final_child_segments`. This
     information is included inside the two corresponding `IntermediateBlockShardInfo`, and they
     signal that the finality for that segment has been reached from the point of view of the
     parent. However, before it can be definitely included in the history of the beacon chain, the
     two blocks from the parent chain with the `IntermediateBlockShardInfo` that included the leaf
     shard information need to also have reached their corresponding
     `FINALITY_PROBABILITY_THRESHOLD` in the beacon chain.
   - Intermediate shard segments will be included to the global history in the beacon chain when the
     the block of the intermediate shard where the `own_segment_info` has reached the
     `FINALITY_PROBABILITY_THRESHOLD`.
   - Finally, beacon chain segments can be included into the global history and into a super segment
     as soon as they are created (as any re-org in the beacon chain will impact the ordering of the
     global history in any case).

7. Once a new beacon chain block confirms the a set of segments from shards in the lower levels are
   final, these segments and any beacon chain segments included in the block form a super segment. A
   Merkle root of these segments is included in the `super_segment_root` field of the
   `SuperSegmentHeader`.
   - To compute the super segment root, segments are ordered starting from the `own_segments_root`
     of the beacon chain, and then in increasing order according to their shard ID and the
     underlying block height in which the segment confirmations were included (e.g.
     `own_segment_info_1`, `own_segment_info_2`, `intermediate_shard_segment_shard1`,
     `child_shard_segment_shard1`, `intermediate_shard_segment_shard2`, etc.).
   - The super segment index is computed is determined by the number of segments aggregated in the
     super segment, and the index of the super segment in the global history of the system. If the
     previous super segment has `super_segment_index` 4 as it aggregates 4 super segments, if the
     next super segments aggregates 5 segments, the `super_segment_index` of the next super segment
     will be 4 + 5 = 9. In this way, the global history index for every segment of the shard is
     assigned.

```rust
struct SuperSegmentHeader {
	// Index of the super segment in the global history of the system
	// (e.g. if the previous segment had super_segment_index 0, and num_segments 4,
	// this super segment will have super_segment_index 4).
	super_segment_index: u64,
	// Beacon height in which the super segment was created. This is useful to inspect the block
	// for additional information about the transactions with segment creations
	beacon_height: BlockNumber,
	// The root of the super segment.
	// It is computed by getting the Merkle root of the tree of segments aggregated in the super segment.
	super_segment_root: Hash,
	// Root hash of the previous super segment that chains all super segments together.
	prev_super_segment_root: Hash,
	// Number of segments aggregated in this super segment.
   // TODO: Num segments not needed because we can look at the block in the beacon height to get the
   // number of segments, their shards and all the actual information.
	num_segments: u64
}
```

8. Farmers in lower-level shards are following the beacon chain and as soon as a new super segment
   is created, the next block producer includes information about it in the block. As soon as a new
   super segment is created, the farmer entitled to create the next block will include a transaction
   that notifies the chain about the new super segment and the list of segments from the shard that
   are part of it and can be conveniently sealed into a `VerifiedSegment` as it has been included in
   the global history. The inclusion of a new `VerifiedSegment` needs to validated as part of the
   consensus rules performed over the block, which means that all validating blocks need to also
   have seen this super segment being created in the beacon chain to accept the block. Sealing a
   segment updates their `UnverifiedSegmentHeader` into a `VerifiedSegmentHeader` and includes the
   `super_segment_root` of the super segment and its index in the global history.

> NOTE: Instead of having to create an ad-hoc transaction to verify a shard segment, in the
> reference implementation we can minimise the information required to verify segments by leveraging
> the beacon chain reference included in shard blocks. If new super segments have been created in
> the beacon chain from the beacon chain block referenced in the last block of the shard and the
> currently being proposed, then all the shard segments for that window can be verified and included
> in the block as `VerifiedSegment`s. This way, we can avoid having to create a transaction for
> every segment that is being verified, and instead just include the segments that are being
> verified in the block body of the shard block.

```rust
pub struct VerifiedSegmentHeader {
   pub super_segment_root: Hash
    /// Index of the segment in the global history in the beacon chain
    pub segment_index: Unaligned<SegmentIndex>,
    /// Reference to the beacon chain block where the super segment for this
    /// segment was created and committed to the global history.
    pub beacon_chain_block: (BlockNumber, BlockHash),
    /// Segment Local Index
    pub local_index: Unaligned<SegmentIndex>,
    /// Root of roots of all records in a segment.
    pub segment_root: SegmentRoot,
    /// Hash of the segment header of the previous segment
    pub prev_segment_header_hash: Blake3Hash,
    /// Last archived block
    pub last_archived_block: LastArchivedBlock,
}
```

## Handling chain re-orgs.

> Note: This is a high-level overview of how re-orgs are handled and why they shouldn't impact
> greatly the operation of shards. Let me know if additional details are needed here and I can
> elaborate a bit more on it.

- When leaf/intermediate shard proposes a new block `block_A`, it will point to a valid beacon chain
  block.
- `blkA` will be submitted to the intermediate's shard parent, in this case the beacon chain, inside
  an `IntermediateShardBlockInformation` data structure.
- This process will be repeated for every new block in the shard: `blkB`, `blkC`, etc.
- Verifying the validity of a shard block is done by making the regular block validity verification
  and all the additional verifications imposed by the hierarchical consensus like checking that the
  referenced beacon chain block is valid. You can already glimpse here how we are not only embedding
  in the longest chain rule local consensus verifications, but also logic involving the overall
  hierarchical consensus operation.
- So far so good. However, all the shards in the system are running a probabilistic consensus. What
  happens if there is a chain re-organisation and a heaviest chain surfaces replacing the current
  longest-chain? Depending on the shard suffering the re-org, the impact is different. Fortunately,
  all of them can be handled quite gracefully through the longest-chain rule.

  - Upward reorgs, meaning leaf or intermediate shards that suffer a re-org, are easily detected. As
    blocks are being submitted to the parent, the parent is able to identify that there is a
    heaviest chain that needs to replace its current view of the child shard. This requires no
    immediate action from the parent apart from this update of the parent's view of the heaviest
    chain.
  - Reorgs from intermediate shards do not involve any immediate action from its child shards. The
    newest heaviest chain may change the way (and specific blocks) where the information about the
    child shard is being included into the parent chain and propagated to the further, but this
    shouldn't have any additional impact.
  - In case of beacon chain reorg, all blocks of the lower level shards that reference stale beacon
    chain blocks automatically become stale too. Lower-level shard blocks must always point to
    beacon chain blocks from the canonical branch.

> Note: How is the beacon chain reference required to validate blocks chosen by farmers in
> lower-level shards?
>
> In terms of protocol correctness, any valid beacon chain block reference can be used, as long as
> it points to a block at a higher or the exact same height and slot than the previous reference in
> the shard. However, to avoid referencing blocks that are too recent (and might not be visible to
> all nodes, increasing the risk of rejection), the reference implementation will use a fixed.
> Specifically, all nodes will aim to propose the most recent valid block they see in their view of
> the beacon chain `1 / SLOT_PROBABILITY` in the past (which is currently set to 1/6, i.e. 6 slots
> which is ~6 seconds). With this, farmers in a shard will try to set their blockchain reference to
> the most recent valid beacon chain block. Failing to include a block reference from the beacon
> chain that is 6 slots behind would mean that the farmer has fell out of sync. This forces shard
> farmers to be as up-to-date as possible with the beacon chain, while keeping as recent as possible
> references in shard block (which will help with the linking and verification of information
> between shards).

## Verifying that the segment roots available are valid and available

> TODO: This section is a work in progress. The high-level idea that we are considering for the
> verification of segments (and their availability) is the following:
>
> - The population of farmers will be periodically assigned to different shards in the system to
>   prevent them from being able to pull attack in the shards they are assigned to through collusion
>   or power dilution.
> - We will leverage the fact that farmers will have to sync to the new shards they are assigned to
>   in every reallocation event to ensure that they verify the most recent segments (and blocks)
>   that haven't been included to the global history and archived. Archived segments will be
>   verified through plotting.
