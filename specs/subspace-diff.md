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

## Dynamic issuance

Dynamic issuance is different (not implemented right now, details will be added later).

## Archiving

Since KZG is no longer used and a farmer still does erasure coding during plotting, archiver was modified to also do
erasure coding of records, so it can commit to erasure coded chunks too. This allows a farmer to generate proofs like
before even though technically record doesn't contain parity chunks. To aid efficient verification of pieces, source and
parity chunks are first committed to separately before combining into record commitment, with parity chunks root also
included in the piece alongside record root.
