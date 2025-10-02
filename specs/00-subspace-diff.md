While the protocol consensus is based on Subspace, but isn't the exact replica. This document attempts to document
important differences when compared to [Subspace specification] until independent complete specification exists.

[Subspace specification]: https://subspace.github.io/protocol-specs/docs/protocol_specifications

# Features removed/not included

## Substrate

The implementation is not tied to the Substrate framework.

## Decoupled execution

Execution is no longer decoupled.

## Votes

Votes are removed, increasing reward frequency can instead be achieved with increased number of shards.

## KZG

KZG is no longer used and was replaced with Merkle Tree.

## Consensus

Root plot public key hash concept for chain bootstrapping was removed. It should not be necessary with more frequent
solution range adjustment and better sync implementation.

# Features changed

## Solution

`Solution` data structure no longer includes reward address in it, reward claiming mechanism is different (not
implemented right now, details will be added later).

`Solution` doesn't have `public_key`, instead it only has `public_key_hash`. The hash is the only thing needed for
consensus anyway and makes the data structure constant size in case other kinds of public keys (PQC) with different
sizes are introduced in the future.

## Chunks

Not being constrained to KZG's 254 bits (31 bytes in practice) scalars, chunk size is now uniformly 32 bytes everywhere.

## Dynamic issuance

Dynamic issuance is different (not implemented right now, details will be added later).

## Archiving

Since KZG is no longer used and a farmer still does erasure coding during plotting, archiver was modified to also do
erasure coding of records, so it can commit to erasure coded chunks too. This allows a farmer to generate proofs like
before even though technically record doesn't contain parity chunks. To aid efficient verification of pieces, source and
parity chunks are first committed to separately before combining into record root, with parity chunks root also
included in the piece alongside record root.

`Segment` data structure is no longer an enum (it is unlikely to need to be changed and even if it does, `SegmentItem`
is already an enum and can support that. `ArchivedBlockProgress` is simplified to a single `u32` (where `0` means block
is complete), resulting in `SegmentHeader` being constant size. `SegmentHeader` was also updated to a data structure
that doesn't have padding bytes in memory, and its in-memory representation is what is being used to derive segment
header hash. All byte lengths in various segment items have changed their encoding from variable-length SCALE encoding
to little-endian `u32` as well, making them fixed size too. As a result, the whole segment construction is now
predictable and deterministic, allowing for efficient piece retrieval, the implementation is simpler and without tricky
edge-cases around variable length encoding.

## Erasure coding

Erasure coding is no longer based on BLS12-381 (related to KZG), instead [reed-solomon-simd] library is used. Not only
this improved performance substantially, this also reduced the need to constrain record chunks to 254 bits (31 bytes in
practice), increasing record size slightly and simplifying a lot of places in the code.

[reed-solomon-simd]: https://github.com/AndersTrier/reed-solomon-simd

Both pieces in a segment and chunks in a sector had source/parity interleaving, which for simplicity was removed. Now
all source pieces/chunks go first, followed by all parity pieces/chunks.

## Global challenge

Global challenge in Subspace was derived by hashing global randomness and slot, but global randomness itself was just a
hash of PoT output. This was changed to instead hash PoT with slot directly, saving one hash and otherwise unnecessary
randomness abstraction.

## Solution range adjustment

Solution range adjustment is more frequent, 300 blocks instead of 2016 (as in Bitcoin).

## PoT iterations adjustment

Number of PoT iterations is adjusted automatically when it is detected that slots are created too frequently on long
enough timescale (which isn't defined at the moment), in contrast to Subspace, where explicit transaction needs to be
submitted to enact such change.

## Terminology

After switching from KZG to Merkle Trees, commitments are renamed to roots, witnesses to proofs. Scalars are also called
record chunks now.

## Proof-of-space

While there are many valid proof-of-space constructions that will successfully validate by Subspace consensus, reference
implementation used one that corresponded to Chia reference implementation and contained all possible proofs. Turns out,
this is unnecessary for Subspace purposes. Because of this, implementation was optimized for performance on both CPU and
GPU at the cost of throwing away some proofs, while ensuring there is still enough of them left to fully encode sectors
during plotting.

In particular, new optimized implementation doesn't sort tables by `y` values, instead it only groups them by buckets,
while limiting the bucket size for performance reasons. Similarly, matches that were found are also truncated for
performance reasons. So as a result, some of the proofs that must exist will not be found.

Since the tables are no longer sorted, proof searching now does full scan of the buckets where matching `y` values are
potentially located, which while is a bit slower, is more than compensated by table creation performance improvements.

Proofs searching has changed to not converting challenge indices to big-endian numbers and moving them around, which
breaks compatibility with Subspace but improves performance due to better spatial locality.
