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

## Terminology

After switching from KZG to Merkle Trees, commitments are renamed to roots, witnesses to proofs. Scalars are also called
record chunks now.
