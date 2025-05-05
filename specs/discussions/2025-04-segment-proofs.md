## Shard segments submission to the beacon chain

The goal of this discussion is to surface the core data structures and mechanics of the process of
committing segments to the global history in the beacon chain, and verifying that a piece of a
segment belongs to the global history in the beacon chain. shard to the upper levels of the
hierarchy.

### Submitting segments to the parent.

1. Child shards create new segments with 128 underlying pieces each with their
   `record || record_root || parity_record_chunks_root || record_proof`. Each child shard creates
   segments independently (in the same way that is currently implemented). Segments are assigned and
   increasing sequence number, `local_index` that determines the order in which they were created in
   created in the shard. These segments that haven't been committed to the global history of in the
   beacon chain are `UnverifiedSegment`s and include their own `UnverifiedSegmentHeader` (which
   matches the current `Segment` and `SegmentHeader` data structures, respectively).

   > TODO: Point to the right parts of the code that implements this for reference.

2. As soon as a new segment has been created in a child shard, it is included in the next block and
   submitted to the parent shard as part of the `consensus_info` inside the shard block. The segment
   information that is propagated to the parent is the following:
   > Note: If all this information is already available as part of the `SegmentHeader`, the raw
   > header can be submitted instead of having to create this ad-hoc `ChildSegmentDescription` data
   > structure.

```rust
struct ChildSegmentDescription {
	// Shard if of the child shard the segment belongs to.
	shard_id: ShardId,
	// The root of the segment.
	segment_root: Hash,
	// Root of the previous segment created in the shard
	prev_segment_root: Hash
	// Local index of the segment (it may be redundant if we assume
	// that segments are always submitted in increasing order)
	local_index: u64,
}
```

3. The status of child shard segments is tracked indexed by their `local_index` through their
   `IndexStatus`. `IndexStatus` gives information about if the segment has been already submitted to
   the parent, is pending confirmation in the beacon chain, or it has been submitted to the global
   history of the beacon chain and has already been assigned its global segment index (pertaining
   its sequence in the global history).

```rust
let segment_index = (local_index: u64, global_index: IndexStatus<u64>)

enum IndexStatus {
	// The segment has been committed to the global history.
	Committed(u64),
	// The segment is pending to be committed to the global history.
	Pending,
	// The segment has not been submitted yet.
	NotSubmitted,
}
```

4. When the child shard block including segments in its `consensus_info` field is submitted to the
   parent chain, the `ChildSegmentDescription` (or `SegmentHeader`) for all segments included in the
   block are lightly verified to see if they are consistent with the view of the history in the
   parent for the child and they can be propagated further up to the beacon chain. The light
   verification performed consists of:

   - Checking that the `prev_segment_root` is equal to the `segment_root` of the previous segment
     for the shard.
   - That the `local_index` for the new segments is the subsequent one of the one for the previous
     segment. As it will be described in future sections, through the data availability layer, nodes
     in the system are periodically checking the correctness of segments being propagated to prevent
     forged segments from being propagated to the beacon chain and requiring a system re-org to
     clean-up forged segments.

5. The parent chain pulls all the `ChildSegmentDescription` (or `SegmentHeader`) from the child
   segments propagated and after performing the corresponding light verification includes them on
   the `consensus_info` of their next block along with any new local segment created in the parent
   shard to propagate them all up to the beacon chain.

6. The submission of a segment to the beacon chain triggers the commitment of all the segments from
   child shards into the global history, and the creation of a `SuperSegment`. `SuperSegments` also
   include all segments from the beacon chain included in the block for which the `SuperSegment` is
   being created. A chain of `SuperSegment`s is used to represent the global history of the system
   in the beacon chain, and each of them be used for efficient verifications of the inclusion of
   segments into the global history.

```rust
/// The global history of the system is represented as a map where for each block of the beacon chain
/// that includes a super segment, the corresponding super segment with information about the list of
/// segments committed is made available on-chain.
let global_history = HashMap<BlockNumber, SuperSegment>

struct SuperSegment {
	// The root of the super segment.
	// It is computed by getting the Merkle root of the tree of segments aggregated in the super segment.
	super_segment_root: Hash,
	// Number of segments aggregated in this super segment.
	num_segments: u64
	// Index of the super segment in the global history of the system
	// (e.g. if the previous segment had super_segment_index 0, and num_segments 4,
	// this super segment will have super_segment_index 4).
	super_segment_index: u64,
	// Beacon height in which the super segment was created. This is useful to inspect the block
	// for additional information about the transactions with segment creations
	beacon_height: BlockNumber,
}
```

7. All child shards are following the beacon chain, and they are monitoring when a new super segment
   is created that includes segments from their shards. When a new super segment is created, they
   will update accordingly their `segment_index` map to point to the right segment index in the
   global history (i.e. `IndexStatus::Committed(global_index)`). This needs to trigger an update in
   the original segment created in the child shard to seal it as final (and part of the global
   history).

8. Additionally, along with updating the global index of the `UnverifiedSegment` of the child shard,
   the `UnverifiedSegment` is transformed into a `SealedSegment` which is the final form of a
   segment that has been committed to the global history. The only difference between an
   `UnverifiedSegment` and a `SealedSegment` is that the `SealedSegment` has been assigned a
   `global_index` from the global history, and that it includes a
   `global_history_proof: SuperSegmentProof` field that can be used to verify given the right super
   segment that this child segment belongs to the global history (further details about the
   generation and verification of these proofs is given in the sections below).

### Generating proofs of segment inclusion

To generate a proof that a specific piece (e.g., `piece1` from `segment4` in `shard11`) is part of
the global history, follow these steps:

1. **Generate the piece inclusion proof**:

   - Retrieve `segment4` from `shard11`.
   - Use the segment's Merkle tree to generate a proof of inclusion for `piece1`.

   ```rust
   fn generate_piece_inclusion_proof(piece: Piece, segment: Segment) -> Option<PieceProof> {
   	 segment.generate_proof(piece)
   }
   ```

2. **Locate the corresponding super segment**:

   - Identify the beacon block that includes the `SuperSegment` containing `segment4`.
   - Retrieve the `SuperSegment` and its Merkle tree.

   ```rust
   fn locate_super_segment(segment: Segment, beacon_chain: BeaconChain) -> Option<SuperSegment> {
   	 beacon_chain.find_super_segment(segment)
   }
   ```

3. **Generate the super segment proof**:

   - Using the `SuperSegment`'s Merkle tree, generate a proof of inclusion for `segment4`.

   ```rust
   fn generate_super_segment_proof(segment: Segment, super_segment: SuperSegment) -> Option<SuperSegmentProof> {
   	 super_segment.generate_proof(segment)
   }
   ```

4. **Combine the proofs**:

   - Package the piece inclusion proof and the super segment proof into a single proof structure.

   ```rust
   struct GlobalHistoryProof {
   	 piece_proof: PieceProof,
   	 super_segment_proof: SuperSegmentProof,
   }

   fn generate_global_history_proof(piece: Piece, segment: Segment, beacon_chain: BeaconChain) -> Option<GlobalHistoryProof> {
   	 let piece_proof = generate_piece_inclusion_proof(piece, segment)?;
   	 let super_segment = locate_super_segment(segment, beacon_chain)?;
   	 let super_segment_proof = generate_super_segment_proof(segment, super_segment)?;
   	 Some(GlobalHistoryProof {
   		  piece_proof,
   		  super_segment_proof,
   	 })
   }
   ```

This process ensures that the proof of inclusion for a piece in the global history is generated by
combining cryptographic proofs from both the segment and the super segment. The resulting proof can
be used to verify the inclusion of the piece in the global history.

### Verifying segment proofs

To verify that a specific piece (e.g., `piece1` from `segment4` in `shard11`) is part of the global
history, follow these steps:

1. **Verify the piece inclusion proof**:

   - Use the Merkle root of `segment4` to validate the inclusion proof for `piece1`.

   ```rust
   fn verify_piece_inclusion_proof(piece: Piece, proof: PieceProof, segment: Segment) -> bool {
   	 segment.verify_proof(piece, proof)
   }
   ```

2. **Verify the super segment proof**:

   - Use the Merkle root of the `SuperSegment` to validate the inclusion proof for `segment4`.

   ```rust
   fn verify_super_segment_proof(segment: Segment, proof: SuperSegmentProof, super_segment: SuperSegment) -> bool {
   	 super_segment.verify_proof(segment, proof)
   }
   ```

3. **Combine the verification steps**:

   - Ensure both the piece inclusion proof and the super segment proof are valid.

   ```rust
   fn verify_global_history_proof(proof: GlobalHistoryProof, piece: Piece, segment: Segment, super_segment: SuperSegment) -> bool {
   	 let piece_valid = verify_piece_inclusion_proof(piece, proof.piece_proof, segment);
   	 let super_segment_valid = verify_super_segment_proof(segment, proof.super_segment_proof, super_segment);
   	 piece_valid && super_segment_valid
   }
   ```

This verification process ensures that the provided proofs are cryptographically valid and that the
piece is indeed part of the global history.

## Genesis segment info

- Genesis segments are created as it is currently implemented for Subspace. With the difference that
  it is already created as a `SealedSegment` in the beacon chain.

> TODO: @nazar-pc, do we need additional information and implementation details for this?
