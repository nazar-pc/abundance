# Sharded Archiving

This is the subprotocol responsible for creating a global canonical history of the whole system, and
of creating the history records that will eventually be archived and become part of farmers' plots.

Shards in the multi-shard Subspace protocol operate independently as parallel chains: producing new
blocks, executing transaction in parallel and independently generating segments of their history
that will eventually become part of the global history stored in the beacon chain.

### Data Structures

This section collects a list of data structures required for the operation of the protocol:

- `segment_commitments[]`: hash map of KZG commitments computed over pieces in a segment and stored
  in the runtime of each shard. This data structure represents the segment history of the shard and
  include both, local segment commitments, and segment commitments from child shards (through
  `SuperSegment`s). Each of them should be accordingly tagged as such.
- `shard_blocks[]`: hash map of blocks committed from child shards. It stores the history of the
  shard in the parent, until these blocks are made available in a new segment.
- `recorded_history_buffer`: Buffer storing segments (i.e. the local history) of a local shard

### Preparing the history of a shard

#### 1. Genesis Block Archiving

- Input: Genesis block of the shard.
- The genesis block is archived as soon as it is produced. We extend the encoding of the genesis
  block with extra pseudorandom data up to `RECORDED_HISTORY_SEGMENT_SIZE`, such that the very first
  archived segment can be produced right away, bootstrapping the farming process. This extra data is
  added to the end of the SCALE-encoded block, so during decoding of the genesis block it'll be
  ignored.
- This becomes the first segment of the shard's history.
- Output: First archived segment.

#### 2. Block History Buffering

- Input: Blocks produced in the shard.
- Once any block produced after genesis becomes `CONFIRMATION_DEPTH_K`-deep, it is included in the
  recorded history. New blocks are added to recorded history that is a buffer of capacity
  `RECORDED_HISTORY_SEGMENT_SIZE`.
- When added to the buffer, blocks are turned into `SegmentItem`s.
- Each segment will have the parent `segment_header` included as the first item. Each
  `segment_header` includes the hash of the previous segment header and the segment commitment of
  the previous segment. Together, segment headers form a chain that is used for quick and efficient
  verification of pieces corresponding to the actual archival history of the blockchain.
- Segment items are combined into segments. Each segment contains at least two of the following
  `SegmentItem`s, in this order:
- The previous segment’s SegmentHeader
  - BlockContinuation, if remained from the previous segment
  - Block(s), as many as fit fully into the current segment, may be none
  - BlockStart, if the next block doesn't fit within the current segment
  - Padding (zero or more) in case padding bytes are necessary to complete the segment to
    `RECORDED_HISTORY_SEGMENT_SIZE`
- Output: Buffered SegmentItems.

#### 3. Block Info Construction

- Input: Blocks produced in the shard.
- When a new block is produced and included to the chain in a shard, farmers (and any other node in
  the system), extend the block using an error correction scheme to generate parity shares:
  1. The block is split into `M = ceil(block_size/l)` shares of `l` bytes.
  2. Each share is arranged into a different column of a `M x l = ceil(block_size/l) x l` matrix.
  3. Shares are erasure coded column-wise with `extend(column, ERASURE_CODING_RATE)`. This
     effectively doubles the number of rows and, hence, the shares per block
     to `total_shares = 2*M`.
  4. Shares themselves are divided into chunks of size `k = NUM_CHUNK_SHARE`, in a way that `l` must
     be multiple of `k`, `l = n x NUM_CHUNK_SHARE`.
  5. For each row, a commitment of the row chunks is created by building a Merkle Proof of the
     individual chunks in the share, `Cbs_i`.
  6. Along with the commitment, a witness is created for each share, `π_i`, that allows to prove
     that a chunk belongs to the share without having to provide all other chunks in the share. This
     is done by adding the relevant path from the root to the chunk in the Merkle tree created for
     the share. Suppose you have a Merkle tree with 4 shares: `(bs1, bs2, bs3, bs4)`, and you want
     to prove that `bs1` belongs to the share. You would need to provide the set of sibling hashes
     `hash(bs2), hash(bs3 || bs4)` as the witness.
  7. All the share commitments are combined into a single commitment for the block, `C_block`, by
     hashing each of the share commitments to `h_i` and interpolating a polynomial over them,
     `C_block = Merkle_Proof(Cbs_0, Cbs_1, ..., Cbs_{total_shares - 1})`.
  8. Total chunks: `total_chunks = total_shares * num_chunks_per_share = 2 * M * (l / k)`.
  9. It is worth noting that the number of shares for each block is dynamic and depends on the
     specific block size. When `block_size` is not a multiple of `l`, the last share will be smaller
     than `l` and padded with zeroes until it reaches size `l`.
- When the commitment for the block is generated, a new `BlockInfo` data structure is generated that
  is used to submit the block information to the parent chain. Any node (not only farmers) are
  allowed to perform this operation.
- New encoded blocks need to be stored by farmers and made available for retrieval by other shards
  and nodes in the system.

```rust
type BlockInfo {
    shard_id: [u8; 20]
    block_height: u64
    block_hash: [u8; 32]
    block_commitment: [u8;32]
}
```

**Table 1: Block Encoding Parameters**

| Parameter              | Description                 | Example Value (Assumed) |
| ---------------------- | --------------------------- | ----------------------- |
| `block_size`           | Size of the block in bytes  | 4,000,000 bytes         |
| `l`                    | Size of each share in bytes | 4 kbytes                |
| `M`                    | Total number of shares      | 100                     |
| `total_shares`         | Shares after erasure coding | 200                     |
| `k`                    | Size of each chunk in bytes | 32 bytes                |
| `num_chunks_per_share` | Chunks per share            | 125                     |
| `total_chunks`         | Total chunks in the block   | 25,000                  |

- Output: Error corrected encoded blocks, and `BlockInfo` to be submitted to the parent shard.

#### 4. Block Info Submission

- Input: `BlockInfo` from a child shard.
- Any node in the shard (from farmers to full-nodes) are allowed to create the `BlockInfo`
  transaction that submits the block information into the parent's shard history.
- The corresponding piece of the block should be stored in the shard and made retrievable via a
  peer-to-peer network protocol (DSN).
- When the transaction in the parent
- Output: Transaction in the parent shard's mempool.

#### 5. Shard block commitment in parent

- Input: Transaction with `BlockInfo` from a child shard.
- The execution of the `BlockInfo` transaction when included in a block of the parent shards commits
  the block from the child shard to the history of the parent by including it on-chain into the
  `shard_blocks[]` table.
- To verify the `BlockInfo`, nodes in the parent shard:
  - Ensure `block_index` is the immediate block after last committed block number in the parent
    shard.
- Output: `BlockInfo` from child shard included in `shard_blocks[]` table of the parent shard.

#### 6. Segment Construction

- Input: Buffered data of size `RAW_RECORD_SIZE`.
- When the buffer (after SCALE-encode) contains enough data to fill a record
  of `RAW_RECORD_SIZE` bytes, it is archived:
  1. Split the record into `d = 2^15 =NUM_CHUNKS` chunks of size `SAFE_BYTES`.
  2. Perform the same operations to extend with parity chunks and commit to the chunks as was done
     for the block. This is done by creating a Merkle tree of the chunks of a record, and committing
     to it.
- Erasure code records column-wise with `extend(column, ERASURE_CODING_RATE)`. This effectively
  doubles the number of rows and thus, records per segment to `NUM_PIECES` (256).
- For each row, a commitment of the row chunks is created by building a Merkle Proof of the
  individual chunks in the share, `Cs_i`, along with their witness `ws_i`.
- For each record, form a `piece = record || record_commitment || record_witness`, `(ri∣∣Ci∣∣πi)`
- Compute the `segment_commitment` by computing the Merkle proof of all the record commitments of
  all rows, the raw records and the extended records
  `C_segment = Merkle_Proof(C0, C1, ..., Cn, C00, Cnn)`.
- Append the `segment_commitment` to the global `segment_commitments[]` table of the chain.
- The segment now consists of `NUM_PIECES` records of roughly 1MiB each
  (`32 bytes * 2^15 chunks + 32 bytes commitment + ~120 bytes witness`), `NUM_PIECES` piece
  commitments, `NUM_PIECES` proofs of 32 bytes each and one 32-byte `segment_commitment`.
- New pieces need to be stored by farmers and made available for retrieval by other shards and nodes
  in the system.
- Pieces from a shard are uniquely identified through an sequentially increasing `piece_index`.
- Output: Segment with `NUM_PIECES` pieces and a `segment_commitment`.

#### 7. Segment Commitment Submission to Beacon Chain

- Input: New segments from a shard.
- As soon as a new block is produced that includes the creation of a new segment, the segment
  commitment is submitted as a transaction in the beacon chain to commit the segment into the global
  history of the system.
- To submit this information to the beacon chain a `SegmentInfo` transaction is created that
  includes the `segment_commitment`, the `segment_index`, and the `shard_id` of the shard where the
  segment was created.
- Any node in the shard (from farmers to full-nodes) are allowed to create the `SegmentInfo`
  transaction that submits the segments into the parent's shard history.

```rust
type SegmentInfo = {
  segment_commitment: Vec<u8>, // 32B
  segment_index: u64, // 8B
  shard_id: ShardId, // 4B
}
```

- Output: `SegmentInfo` transaction in the beacon chain mempool.

#### 8. Segment commitment in beacon chain and Super Segment Construction

- Input: Transaction with `SegmentInfo` in the beacon chain.
- The execution of the `SegmentInfo` transaction when included in a block of the beacon chain
  triggers the commitment of the shard's segment in the global history of the beacon chain.
- To verify the `SegmentInfo` and execute the transaction, nodes verify the following:
  - Ensure `segment_index` is the subsequent index after the last one committed for the shard.
- If the verification of the `SegmentInfo` is successful, the child shard segment is added to the
  beacon chain shard's history by adding the shard's `segment_commitment` to the
  `segment_commitments[]` that indexes the all commitments from the global history (including beacon
  chain and all child segments).
- Apart from adding the segment commitment in the global history, a new `SuperSegment` is created
  aggregating all the segment commitments included in a block of the beacon chain. The goal for this
  super segment commitment is to simplify the verification of segments by light clients.
- The `super_segment` is created by aggregating the segment commitments of the child segments and
  creating a new commitment that is included in the parent shard's history committing (and wrapping)
  all the history of shards in the lower layers of the architecture. This `super_segment` commitment
  is directly a Merkle proof of all of the shard segments and child super segment commitments from
  that shard: `C_super_segment = Merkle_Proof(S1, ..., Sn, SS1, ..., SSn)`.
- Along with `C_super_segment` the corresponding `ws1` segments commitment are created.
- When a new super segment is created instead of broadcasting the network the full super segment,
  only the `SuperSegmentHeader` is broadcasted, allowing nodes to proactively determine if they
  require the full super segment data.
- The `SuperSegmentHeader` includes the following fields:
  - `history_delta`: Number of segment commitments included in the `super_segment`. This will be
    used to determine the number of segments committed in the beacon chain at a specific
    `block_height`.
  - `super_segment_commitment` commitment of the super segment that implicitly verifies into the
    global history all the included segments.
  - `block_height` in the beacon chain where the super segment was created. This is used to identify
    the block in the child shard that contains the segments included in the super segment and pull
    all the super segment data (if needed).
- Output: Add segment commitments from children to `segment_commitments[]`, and a new `SuperSegment`
  per block with segment commitments.

```rust
type SuperSegmentHeader = {
	super_segment_commitment: Vec<u8>,
  block_height: u64,
	history_delta: u64,
};
```

#### 9. Recursive Local Block and Segment Creation and Submission

- Input: Transactions with `BlockInfo`, new local segments, and `SegmentInfo`.
- The protocol is recursive, which means that if we have a hierarchical architecture with more than
  one level below the beacon chain, all shards independently of the level they belong to will
  perform the same operations described in the sections from above: i.e. submit new
  [`BlockInfo`](#3-block-info-construction) to their parents as new blocks are created, creating
  [new segments](#2-block-history-buffering) with their local histories, and the commitment of
  [segments to the beacon chain](#7-segment-commitment-submission-to-beacon-chain).

#### 10. Global history in the beacon chain

- Input: Transactions with `SegmentInfo` from all children, and `BlockInfo` from immediate children
- The protocol operates in the same way in the beacon chain as it does for any other parent in the
  network. Nodes participating in the immediate children shards are periodically submitting
  transactions with`BlockInfo`. The process of committing these to the global history is analogous
  to how it was done in parents from lower layers:
  - The creation of a new block in the beacon chain including `BlockInfo` transactions appends the
    child block information to the `shard_blocks[]` table of the beacon chain.
- The only difference with other parents in the network, is that the beacon chain also receives
  `SegmentInfo` transactions from any shard in the network to commit their segments into the global
  history in the system (as described
  [in this section](#segment-commitment-in-beacon-chain-and-super-segment-construction)).
- Output: Ordered global history is `segment_commitment[]` and `SuperSegments`.
