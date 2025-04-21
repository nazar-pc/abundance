# Data Availability Sampling

Child shards are propagating to the upper layers of the system commitments of their blocks through
`BlockInfo`, and segment commitments to the beacon chain through `SegmentInfo` transactions.
However, in order to be able to verify these blocks and segments, and to ensure that segments are
always available for plotting, they all must be available for retrieval.

This is done through a data availability sampling protocol that allows any node in the system to
verify that blocks and segments are available for retrieval. The protocol is based on the idea of
sampling a small number of chunks from encoded segments and blocks and verifying that they are
available.

### Availability sampling.

This protocol is used to sample both: blocks and segments in shards.

#### 1. Sampling process

The sampling process is the same for both, blocks and segments. Any node in the system, but in
particular light nodes, periodically sample random blocks and segments from different shards. We
expect nodes of any kind to prioritise sampling the shards that they are interested in, but they are
all free to choose where they sample.

The sampling process is as follows:

1.  **Select a Chunk**:
    - Choose a random share index `i` from `0` to `total_shares - 1`.
    - Choose a random chunk index `j` from `0` to `num_chunks_per_share - 1`.
2.  **Request Data**:
    - Request chunk `j` from share `i` and its witness `π_{i,j}` from a node (e.g., farmer).
    - The witness should include at least:
      - Intra-share Merkle path from chunk `j` to `Cbs_i` (`Cs_i` for segments).
      - Inter-share Merkle path from `Cbs_i` to `C_block` (`Cs_i` and `segment_commitment` for
        segments).
    - Segment sample verification can also optionally include a witness to verify that the segment
      is included in the global history in the beacon chain through the `super_segment_commitment`
      for that shard segment and its corresponding witness.
    - Requests are broadcast to the network, they are not directed to a specific node.
3.  **Repeat**:
    - Perform multiple sampling rounds to build confidence.

#### 2. Verification

For each sampled chunk:

- Verify the witness `π_{i,j}` by:
  - Hashing the chunk data with the intra-share Merkle path to compute `Cbs_i` (`Cs_i` for
    segments).
  - Hashing `Cbs_i` with the inter-share Merkle path to compute `C_block` (`Cs_i` and
    `segment_commitment` for segments).
- Compare the computed `C_block` with the trusted `C_block` from the `BlockInfo` on the parent
  chain. For segments, compare with the `segment_commitment` from the `SegmentInfo` transaction in
  the beacon chain.
- **Outcome**:
  - **Success**: Chunk is available and authentic.
  - **Failure**: Chunk is missing or the proof is invalid, indicating potential unavailability.
- All outcomes are signed and gossiped to the network so nodes can leverage information from others
  and create their own view of the availability of blocks and segments.

**Table 2: Verification Steps**

| Step                     | Action                                        | Outcome if Failed   |
| ------------------------ | --------------------------------------------- | ------------------- |
| Compute intra-share path | Hash chunk with Merkle path to get `Cs_i`     | Invalid proof       |
| Compute inter-share path | Hash `Cs_i` with Merkle path to get `C_block` | Invalid proof       |
| Compare `C_block`        | Match with trusted `C_block`                  | Chunk not authentic |
| Check chunk retrieval    | Ensure chunk data is provided                 | Chunk unavailable   |

#### 3. Broadcast channels

The data availability sampling protocol relies in the implementation of different broadcast channels
used by nodes to request sampling information for specific chunks in a shard and to report the
availability of chunks. Concretely, the following broadcast channels are used:

- **Shard Sampling Topic**: Used to request sampling information for specific chunks in a shard, and
  to report the availability of specific chunks. There is one independent topic per shard, and only
  nodes interested in participating in the data sampling for that shard are expected to be
  subscribed to it. These are the types of messages that are expected to be sent in this topic:
  - `SAMPLE_REQUEST`: Request for a specific chunk in a shard. This message should include the chunk
    index and the share index.
  - `SAMPLE_RESPONSE`: Response to a sampling request. This message should include the chunk data
    and the witness for the chunk. The fact that all nodes interested in the shard will be listening
    to `SAMPLE_REQUEST` messages allow them to implicitly benefit from the sampling being performed
    by other nodes (if they choose to verify those messages themselves).
- **Unavailability Report Topic**: Used to report the unavailability of chunks in a shard. This is a
  global topic that all nodes in the system are expected to be subscribed to (i.e. all nodes
  participating in the beacon chain). This allows any node in the system to report the
  unavailability of chunks in any shard, and to receive reports from other nodes. These are the
  types of messages that are expected to be sent in this topic:
  - `UNAVAILABILITY_REPORT`: Report for a specific chunk in a shard. This message should include the
    chunk index and the share index, and it should be signed by the node reporting the
    unavailability.
  - This topic implements reputational schemes to prevent spamming and forged reports. Nodes that
    report unavailability are expected to be penalised if the report is found to be false by other
    nodes dropping their connections to them (and eventually isolating them from the network).

#### 5. Confidence Level and Sampling Rounds

The goal is to achieve high confidence (e.g., 99%) that the block is available, meaning less than a
small fraction (e.g., 1%) of data is missing. This is based on statistical sampling, as it is the
case in other data availability-specific projects like Celestia or Avail.

- **Parameters**:
  - `T = total_chunks`: Total chunks in the block.
  - `f`: Maximum fraction of missing data to detect (e.g., 0.01).
  - `p`: Desired confidence level (e.g., 0.99).
- **Formula**:
  - Probability all `k` samples are available if fraction `f` is missing: `(1 - f)^k`.
  - To ensure `(1 - f)^k < 1 - p`, solve for `k`: [ k > \frac{\ln(1 - p)}{\ln(1 - f)} ]
  - Example: For `f = 0.01`, `p = 0.99`: [ k > \frac{\ln(0.01)}{\ln(0.99)} \approx 460 ]
- **Implementation**:
  - Sample `k` unique chunks or use a dynamic stopping rule (e.g., stop after `k` successful samples
    or if failures exceed a threshold).
  - Adjust `k` based on block size and network conditions.

**Table 3: Sampling Requirements**

> NOTE: The numbers for this table are just placeholders and will need to be appropriately adjusted
> when the final parameters for the protocol are considered.

| Confidence (`p`) | Max Missing Fraction (`f`) | Required Samples (`k`) |
| ---------------- | -------------------------- | ---------------------- |
| 0.99             | 0.01                       | ~460                   |
| 0.99             | 0.05                       | ~90                    |
| 0.95             | 0.01                       | ~300                   |

#### 6. Unavailability Reporting

Unavailability reporting is a critical mechanism to ensure the integrity and reliability of the data
availability sampling protocol. When sampling indicates potential unavailability (e.g., multiple
failed samples), nodes in the network must take coordinated actions to report and address the issue.

- Unavailability reporting is triggered when a node detects that a block or segment is potentially
  unavailable based on the sampling process.

  1. **Report Content**:

  - The report must include the following details:
    - `shard_id`, `block_height`, and `block_hash` for blocks, and `shard_id`, `segment_index` and
      `segment_commitment` for segments: These are obtained from the `BlockInfo` and `SegmentInfo`
      of the block and segments in question, respectively, uniquely identifying the block in a
      shard.
    - Failed Share/Chunk Indices: A list of the specific shares or chunks that failed during the
      sampling process. The number of indices reported should be such that there is no way of
      recovering the block (segment) from error corrected shares.

  2. **Reporting Mechanism**:

  - The unavailability report is signed by the reporting node and broadcast to the network using the
    **Unavailability Report Topic**.
    - This ensures that all nodes in the system are informed of the potential issue.
  - **Submission to the Beacon Chain**:
    - The report is also submitted to the beacon chain, where it can trigger further consensus
      actions.

  3. **Unavailability Verification**:

  - When a farmer from the beacon chain observes an unavailability report, they include the report
    in the next block they farm.
  - This opens a verification window of `SOFT_UNAVAILABILITY_WINDOW` blocks where subsequent farmers
    in the beacon chain can choose to verify the report and either accept it or reject it.
  - Throughout the `SOFT_UNAVAILABILITY_WINDOW`, farmers are not obliged to vote on the report, but
    they are encouraged to do so to ensure the integrity of the network.
  - In the next block after the `SOFT_UNAVAILABILITY_WINDOW`, all the voted for the unavailability
    of the block or the segments are considered, and if `2/3` of the votes are in favor of accepting
    the unavailability report (assuming at least `1/3` of the farmers proposing blocks in the
    `SOFT_UNAVAILABILITY_WINDOW` have voted), the report is accepted, and consensus actions are
    triggered to recover the unavailability (and penalise the shard accordingly).
  - This process ensures that unavailability reports are validated and acted upon in a timely
    manner.
  - It may be the case that the unavailable sample is recovered while `SOFT_UNAVAILABLE_WINDOW` is
    active, in which case there may be votes in favor of the unavailability report at the beginning
    of the window, and negative votes from there on.
  - After the `SOFT_UNAVAILABILITY_WINDOW` is over, a new `HARD_UNAVAILABILITY_WINDOW` timer is
    triggered to gather additional votes for the report if the previous voting was unsuccessful. The
    voting works in the same way, and if the report ends up being accepted, the block/segment
    reported must be considered unavailable.

  > Note: `SOFT_UNAVAILABILITY_WINDOW` and `HARD_UNAVAILABILITY_WINDOW` are parameters that can be
  > adjusted based on the network's requirements and the expected frequency of unavailability.
  > `SOFT_UNAVAILABILITY_WINDOW should be adjusted as the time it takes for a block or segment to be recovered, and `HARD_UNAVAILABILITY_WINDOW`
  > as the grace period before actions in face of an unavailability are triggered.

  4. **Sample Recovery**:

  - In parallel with the unavailability verification, nodes in the network can attempt to recover
    the missing shares or chunks by:
    - Requesting the missing shares from other nodes in the network.
    - Using erasure coding techniques to reconstruct the missing data from available shares.
    - Leveraging the system's DSN (Decentralised Storage Network) to retrieve the missing shares
      from other nodes (see sections below).

  5. **Unavailability Penalties**:

  - When the unavailability of a sample is confirmed, the following actions are taken to penalise
    participants of the shard where the sample is unavailable:
    - Farmers of that shard are penalised by losing the plots assigned to that shard that include
      the unavailable segment (this essentially reduced the power of a farmer in the shard, and
      therefore the probability of being selected to farm a block in that shard).
    - This will force these farmers to re-plot if they want to continue farming in that shard, and
      will also trigger re-plotting for other farmers that have that segments on their plot,
      rebalancing in this way the history.
    - This is also a great way to flag the system when a shard is losing power, triggering others
      farmers to potentially prioritise that farm to rebalance the power in the system.

  6. **Preventing Abuse and Ensuring Accuracy**: To maintain the integrity of the unavailability
     reporting process, the following measures are implemented:

  - Signature Verification: All unavailability reports must be signed by the reporting node. This
    ensures accountability and prevents anonymous or malicious reports.
  - Double-Checking: When a node receives an unavailability report, it independently verifies the
    claims by performing its own sampling of the reported block or segment.
  - Challenge Window: A challenge window is provided during which other nodes can dispute the
    unavailability report. This prevents collusion and ensures that false reports are identified and
    rejected.
  - Threshold-Based Validation: Reports are only considered valid if they meet a predefined
    threshold of supporting evidence (e.g., 2/3 nodes independently confirming the unavailability).

By implementing these measures, the unavailability reporting protocol ensures that the network can
quickly and effectively address data availability issues while maintaining trust and accountability
among participants.

#### 7. Recovery

Erasure coding enables block reconstruction if enough shares are available:

- **Requirement**:
  - For a (2M, M) erasure code, at least `M` out of `2 * M` shares are needed.
- **Process**:
  - Identify available shares via sampling or direct requests.
  - Request missing shares from other nodes (e.g., full-nodes and farmers).
  - Use erasure decoding to reconstruct the block.
- **Fallback**:
  - If fewer than `M` shares are available, flag the block as unrecoverable and escalate to
    consensus mechanisms (e.g., retransmission).

**Table 4: Recovery Scenarios**

| Available Shares | Outcome                    | Action                         |
| ---------------- | -------------------------- | ------------------------------ |
| ≥ M              | Block can be reconstructed | Retrieve shares, decode        |
| < M              | Block unrecoverable        | Report, request retransmission |
