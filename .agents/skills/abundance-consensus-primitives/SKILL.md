---
name: abundance-consensus-primitives
description: Design intent behind the blockchain structure of the `abundance` repository. Use when working on blocks, headers, bodies, segments, super segments, archiving, Merkle trees, consensus constants or sync in `ab-core-primitives`, `ab-merkle-tree`, `ab-archiving` or the `ab-client-*` crates, and before reporting a bug or proposing a change to block formats, parsers, builders or commitment structure there.
---

# Consensus primitives: design intent

Read `CONTRIBUTING.md` first, this only adds what is specific to blocks, archiving and Merkle trees. Rustdoc on the
items themselves is up to date and authoritative, so check it before assuming something is undocumented. Background on
why things look the way they do is in blog posts under `website/main/content/blog/` (for example
`2025-03-31-hierarchical-subspace-protocol.md`, `2025-05-19-what-does-a-block-look-like.md`,
`2025-04-28-proving-blocks-and-segments.md` and the two super segments posts from 2026), and `specs/00-subspace-diff.md`
lists differences from Subspace. The posts are historical, the code wins where they disagree.

## The system is mostly not built yet

The design is a hierarchy: the beacon chain at the top, intermediate shards below it, leaf shards below those. Every
chain archives its own history into local segments (`LocalSegmentIndex`). Segment roots propagate up to the beacon
chain, which orders them globally and, once confirmed, combines them into super segments.

Only the beacon chain is implemented, and only partially. Intermediate and leaf shard blocks have formats, parsers and
builders, but nothing produces or imports them yet. So "nothing uses this field", "this check can't fail today" or "this
layout is overkill for current usage" is not an argument for removing or simplifying something. Judge a design by what
the full hierarchy needs, and ask when that isn't clear.

## Why the commitment structure looks like this

The whole point of the structure is recursive verifiability: anyone must be able to prove, with a short proof, claims
like "this leaf shard block didn't commit to this segment" or "this intermediate shard block included wrong data",
against a header from the level above. Cross-shard communication will prove leaf shard transactions through the beacon
chain. This drives several choices that look odd in isolation:

- Proofs go against the block root, a Merkle root over header fields, rather than a hash of the serialized header, so
  proving one field doesn't require the whole header.
- Segment roots are not stored in headers. They are in bodies and proven against the header through the body root.
  Intermediate shards also commit to the shard index and local segment index of leaf shard segments, so data seen on the
  beacon chain can be verified against an intermediate shard header without leaf shard headers. Where hashing doesn't
  follow the byte layout literally (shard index mixed into segment roots, for example), that is deliberate.
- Some header fields duplicate data that is also committed in the body (the roots of child shard block roots, see their
  rustdoc). This redundancy is on purpose, to keep proofs through the header short. Don't propose removing it, and don't
  propose dropping the same data from the body root either.
- Header size matters for light clients, so adding fields to headers needs a strong reason.

## Parsers, builders and owned types

- The layout comments at the top of `try_from_bytes()` in `block/header.rs` and `block/body.rs` are the specification.
  When a builder and a parser disagree, the builder is changed to match the parser. Encoding is canonical: parsers
  accept exactly the bytes builders write, which is why `to_owned()` can just copy the parsed bytes into an owned buffer
  and the single `.expect()` there holds.
- Padding is computed from the memory address of the bytes, not from their offset in the slice, so that a body can be
  parsed and used in place inside a larger allocation (a memory-mapped file, a network buffer) without copying. Owned
  buffers are 16-byte aligned for the same reason. This is not a bug in offset arithmetic.
- There is one, checked, parsing path. Unchecked parsers of blocks and bodies were removed after measuring that the
  checks cost about as much as hashing the same bytes, while the duplicated logic made block APIs harder to follow.
  Don't reintroduce `*_unchecked` twins without a measured, meaningful gain. If one is ever justified, it may only skip
  compute-heavy consistency checks on trusted data (read back from the database after a checksum), never anything whose
  absence could cause unsafety or a panic.
- Owned types (`Owned*Header`, `Owned*Body`, `Owned*Block`) hold a `SharedAlignedBuffer` with the borrowed view attached
  through `Yoke`, and cache roots/hashes so they are computed once. `to_owned()` is meant to be cheap and infallible.
- A parsed view may keep the bytes it was parsed from and hash them in one go, with its fields read-only
  (`pub mut(self)`) so they can't diverge from those bytes, like `BlockHeaderConsensusParameters`. Such a view only
  comes from its parser, and builders take the owned counterpart (`OwnedBlockHeaderConsensusParameters`). Hashing the
  encoding instead of feeding fields one by one keeps the hash unambiguous (presence flags included) without a second
  encoder that can drift from the parser.
- Size limits are a consensus matter and will live in `ConsensusConstants` (a maximum block size of a few MB that will
  grow over time). They are checked on raw bytes at the untrusted boundary where blocks come from the network, not
  during parsing. Parsers stay size-agnostic except for structural bounds, such as rejecting bodies longer than
  `u32::MAX` bytes because an owned buffer can't hold them. Such bounds stay even where they "can't be hit today".

## Merkle trees

`ab-merkle-tree` is a generic, domain-free crate: no protocol-specific keys, tree aliases or size policies in it. Those
live in `ab_core_primitives::trees`, where every tree has its own BLAKE3 key. Rules behind the current API:

- Composability matters: with a hasher that doesn't depend on tree size, a tree of trees is the same as one larger tree,
  and segments, MMRs and farmer commitments rely on that.
- Commit to the number of leaves only when it isn't known at compile time. Fixed-size trees use a static key,
  runtime-sized ones derive the key from the size. Proposals to commit to the size everywhere were rejected. The MMR
  appends, so it can't depend on the size at all.
- Trees that need the number of leaves upfront take `TrustedLen` iterators. Prefer the bound style
  `Iter: IntoIterator<Item = ..., IntoIter: TrustedLen>`.
- BLAKE3 economics: two 32-byte nodes fill exactly one 64-byte BLAKE3 block, and keyed hashing makes domain separation
  free. Anything that pre-hashes, prefixes a size or otherwise adds bytes to a node pair costs an extra compression per
  node, so it needs a very good reason. Batch APIs in `ab-blake3` (`single_block_keyed_hash_many_exact()`, `const`
  variants) already exist, check before adding new ones.

## Super segments and shard segments

Rustdoc of `derive_super_segment_for_block()` and `check_shard_segments()` in `ab-client-consensus-common` has the
details, this is the intent behind them:

- Beacon chain blocks list shard segments: own segments of intermediate shard blocks and segments of their leaf
  shards. Those listed in the base block `n - block_confirmation_depth - shard_confirmation_depth` are candidates for
  the super segment of block `n`. Listing a segment of a shard again in one of the `shard_confirmation_depth` blocks
  after the base means that shard reorged, which postpones the candidate at the lowest index listed again and every
  candidate of that shard above it. The rest are confirmed and go into the super segment.
- A confirmed shard segment can't be listed again, otherwise it would end up in super segments twice. Indices of each
  shard are gap-free: the first segments of a shard in a block continue its most recent listing on the block's own fork
  or go back to list segments again after a shard reorg, but never below the confirmed ones, and later segments of the
  same shard in that block continue exactly. Going back within one block isn't allowed, the builder puts such a reorg
  into the next block.
- Recent blocks may differ between forks, so they are always read through the ancestry of the block being verified
  (`shard_segment_roots()` takes a descendant root), never by number on the best chain. Super segments themselves don't
  depend on the fork, their base block and the blocks after it are confirmed.
- The highest confirmed local segment index of each shard is consensus state that belongs to beacon chain state, which
  doesn't exist yet. Until then the database derives it from persisted super segments, and Snap sync has no verifiable
  way to get it (see the `TODO` in the database).

## Archiving and sync

- Archiver code in `ab-archiving` and `ab-client-archiving` derives from single-chain Subspace. Conditions that only
  make sense there (several sync strategies, Substrate-era finalization comments, assumptions that every chain is the
  beacon chain) are artifacts to fix, not intent to preserve. Only the beacon chain genesis block is padded to a full
  segment.
- There is exactly one sync mechanism, "Snap sync", which is not implemented yet. Call it that, not "special sync" or
  "fast sync". Its contract, which archiver tests already model: all known segment headers are stored first, then a
  block from the end of the last known segment is inserted directly without its parent.

## Terminology and constants

- Blocks are "confirmed" at `block_confirmation_depth`, never "final" or "finalized" (code under `subspace/` and blog
  posts are left as is).
- "Block number" is the height in the chain and nothing else. A position relative to the best block is a "block offset",
  a position among forks at the same height is a "fork offset". "Next best block", not "next block".
- Confirmation-related constants are `*_confirmation_depth` (`block_confirmation_depth`, `shard_confirmation_depth`, and
  `soft_confirmation_depth` on the database side, which only controls when blocks are written to disk). The database and
  other client crates take the individual values they need instead of the whole `ConsensusConstants`, to make mocking
  easier and keep arguments minimal.
- Several values in `ab-node`'s chain spec are reduced 10x for testing, with the production value in a `TODO` comment
  next to them. Reason about relationships between constants (such as whether one depth exceeds another) with both sets
  in mind, and don't call the test values bugs.
