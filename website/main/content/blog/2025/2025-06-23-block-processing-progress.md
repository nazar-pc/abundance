---
title: Block processing progress
date: 2025-06-23
draft: false
description: Some improvements to block import and adventures with GPU plotting
tags: [ status-update ]
authors: [ nazar-pc ]
---

This was a lighter week on meaningful changes, but there are still few things to share. First of all, last week's block
import was (and still is) and incomplete prototype, but this week it was extended to become a bit more complete. I was
also working on bringing up more components frm Subspace, including farmer, which lead me to attempt GPU plotting in
Rust, so let's get into it.

<!--more-->

## Block processing progress

The design of the block import is such that allows for completely parallel import of even interdependent blocks, but
they need to be scheduled sequentially, such that parent header is seen by the block import before the next block comes.
Well, buffering of these parent headers was not implemented originally, but [PR 297] finally implemented that in a
fairly efficient way. Then [PR 298] took advantage of the fact that there should be very few blocks buffered and the
most common case is to reference the last scheduled block to replace a map with a much simpler `VecDeque`.

[PR 297]: https://github.com/nazar-pc/abundance/pull/297

[PR 298]: https://github.com/nazar-pc/abundance/pull/298

Another thing that was missing was Merkle Mountain Range root. The primary reason for that was that MMR itself was not
implemented. I have implemented MMR (which is a more generic version of unbalanced Merkle Tree, which is in turn more
generic version of balanced Merkle Tree) in [PR 299]. There is a bunch of tests, but more tests and some benchmarks
still need to be added. With that [PR 300] implemented maintenance and checking for MMR in block import. This required
some changes to client API and block buffering (MMR for a block needed to be created before its import starts, see
parallel block import above).

[PR 299]: https://github.com/nazar-pc/abundance/pull/299

[PR 300]: https://github.com/nazar-pc/abundance/pull/300

Another smaller thing was timestamp in the header, which together with `BlockTimestamp` new type introduction was
implemented in [PR 301] in the spirit of how it is done in Substrate's pallet-timestamp.

[PR 301]: https://github.com/nazar-pc/abundance/pull/301

After looking at block structure closely and thinking about future protocol upgrades, I ended up removing `version`
field from block header in [PR 302], I no longer think it'll be necessary.

[PR 302]: https://github.com/nazar-pc/abundance/pull/302

## Getting closer to having a farmer

In order for blocks to exist, they need to be created, which implies creation of solution, which will need a farmer. So
I looked at dependency tree and extracted both `ab-data-retrieval` and `ab-farmer-components` in [PR 292] as modified
versions of their counterparts from Subspace codebase.

[PR 292]: https://github.com/nazar-pc/abundance/pull/292

This should technically be sufficient to plot and farm blocks in some test environment, but `subspace-farmer` has more
heavy dependencies before it can become `ab-farmer`. Some like networking stack will likely remain largely the same, but
GPU plotting that was broken for some time due to getting rid of KZG needs a replacement.

I looked at [CubeCL] more closely and tried to prototype something, but was immediately disappointed with several
things. First of all, it only works with data structures and functions/methods that are annotated with its macros, so
forget about using basically anything that exists on crates.io 😞.

[CubeCL]: https://github.com/tracel-ai/cubecl

Well, the use case is somewhat narrow, so I wouldn't really need many dependencies out of my control to begin with, so I
decided to start with writing a simple kernel that creates a ChaCha8 keystream only to be disappointed again by the fact
that I can't use `[u32; 16]` as an input to the kernel and workarounds are quite ugly 😭

I then looked again at [rust-gpu] more closely. The benefit of it is that it does seem to compile normal Rust code,
meaning using external dependencies not aware of rust-gpu should generally be fine, but then I discovered that it is a
compiler backend and is tied to a specific version of nightly rustc, which is from more than a year ago. There is a PR
to update it to [something from this year], but it makes slow progress and is already substantially older than what I'm
targeting already. At this time I' not ready to commit to old compiler versions and even if it is only used for
compiling GPU code it'll be problematic due to various unstable features used.

[rust-gpu]: https://github.com/Rust-GPU/rust-gpu

[something from this year]: https://github.com/Rust-GPU/rust-gpu/pull/249

Overall, rust-gpu seems like a more straightforward and Rust-like design, while CubeCL has interesting features that may
result in higher performance at the cost of Rust-but-not-really DSL, but neither of them work the way I expected them,
which is a bummer. I'll probably try again with CubeCL again this week to see how far I can go before pulling my hair
out.

## Other things

There were some CI improvement: [PR 303] extended checks to make sure all crates can compile separately with default
feature set, [PR 304] increased CI concurrency to make things finish faster overall. I could make it even faster, but
that would require ugly workarounds due to [GitHub Actions limitation], so I abandoned that idea for now.

[PR 303]: https://github.com/nazar-pc/abundance/pull/303

[PR 304]: https://github.com/nazar-pc/abundance/pull/304

[GitHub Actions limitation]: https://github.com/orgs/community/discussions/163715

[PR 296] introduced block root caching since otherwise it would have been recomputed every time `BlockHeader::root()` is
called, which is fairly common. [PR 307] improved balanced Merkle Tree performance slightly and reduced amount of
`unsafe` there but a tiny amount.

[PR 296]: https://github.com/nazar-pc/abundance/pull/296

[PR 307]: https://github.com/nazar-pc/abundance/pull/307

[PR 300] already reduced owned block data structures by wrapping internals with `Arc` since some of these (like header)
are long-living and will be cloned many times, which [PR 309] followed-up with usage of [rclite], offering an even more
compact `Arc` implementation.

[PR 309]: https://github.com/nazar-pc/abundance/pull/309

[rclite]: https://github.com/fereidani/rclite

## Upcoming plans

I spent some time looking around and thinking about state management, Sparse Merkle Tree implementations and may start
implementing something this week. This work is also a pre-requisite for block execution, which needs to deal with state
and commit to state root.

I'd like to work with CubeCL some more as well, hopefully implementing GPU plotting that may not be very fast, but at
least in the right overall shape to move forward with farmer CLI.

In short, still a lot of work ahead, but steady progress is made every week, and you can find me on [Zulip] in case of
any questions.

[Zulip]: https://abundance.zulipchat.com/
