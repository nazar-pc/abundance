---
title: Client database prototype
date: 2025-08-01
draft: false
description: "Progress on client database implementation and other work over the last week or so"
tags: [ status-update ]
authors: [ nazar-pc ]
---

The biggest update since the last blog post is that an initial prototype of the database was merged. It lays the
foundation in terms of fundamental architecture and will now be extended to support more features. There were also
updates in a few other areas.

<!--more-->

## Client database

[PR 348] finally landed an initial prototype of the client database that I described [in the last update]. It focuses on
covering existing apis for storing blocks and querying block headers and related stuff. There will be more work needed
to read full blocks from the database and a bunch of complexity around state handling, which I think I have mostly
figured out, just not implemented yet.

[PR 348]: https://github.com/nazar-pc/abundance/pull/348

[in the last update]: ../2025-07-20-sparse-merkle-tree-and-client-database-preparation.md

You can see the last update and PR description for more details. Despite how incomplete it is, I'm happy it landed and
can finally build on top of that foundation.

## GPU plotting implementation

After more upstream improvements in `rust-gpu`, most notably [better const folding for int and bool], I was able to
finish and merge implementation of `compute_fn()` shaders in [PR 343]. There are still a few key components missing like
matching, sorting and hopefully erasure coding, before it can be assembled into the complete functional thing, but this
is a good progress getting us closer to the goal.

[better const folding for int and bool]: https://github.com/Rust-GPU/rust-gpu/pull/317

[PR 343]: https://github.com/nazar-pc/abundance/pull/343

## Improved Merkle Tree performance

I think I mentioned upstream BLAKE3 issues/feature requests a few times already, but this time I used `blake3`'s
private/undocumented API in [PR 344] to implement a version of [SIMD-accelerated hashing of multiple values] in
`ab-blake3`, specifically block-sized values. I then used it to construct a balanced Merkle Tree in [PR 345] and saw ~5x
speedup, though it is still over 2x away from the theoretical performance of BLAKE3 on my hardware. I believe a large
fraction of the gap is due to the private API, which was not designed for this use case.

[PR 344]: https://github.com/nazar-pc/abundance/pull/344

[SIMD-accelerated hashing of multiple values]: https://github.com/BLAKE3-team/BLAKE3/issues/478

[PR 345]: https://github.com/nazar-pc/abundance/pull/345

So there is still a lot higher performance on the table, and only full construction of the balanced Merkle Tree was
optimized for now, with more to come later with incremental updates.

## Other updates

In other news, Alfonso's contract with Subspace Foundation has concluded at the end of July, so his time availability
will likely decrease. We made decent progress on the consensus side from a theoretical perspective, and I should have
most of the answers needed to work on implementation of the hierarchical sharded version of the protocol soon.

I think the design is quite elegant overall, given the complexity of the design space. Once solidified, I'll update the
book with more details for others to read it. We initially focused on a version that is more for those who are already
familiar with Subspace, but a standalone version from the first principles will be eventually needed as well.

## Upcoming plans

I'll keep this update short and focused on key items.

The next steps include implementing more features in the client database, notably around state management. Probably
after some code refactoring to make `lib.rs` a bit more manageable.

The GPU plotting implementation still needs more work, which I'll probably interleave among other things. I'm also
considering using CPU stubs for some components in the meantime now that I have confidence in the feasibility of the
approach with `rust-gpu`, just to get the farmer migrated over sooner.

With more state-related updates, single-node block production should be getting closer to reality, but I think it'll be
at least several updates until then.

I'm also considering publishing `ab-blake3` and maybe other components to crates.io, so they can get attention and be
useful to a wider community instead of being siloed in the `abundance` repo.

The development progresses with discussions on [Zulip], feel free to join and ask any questions you might have.

[Zulip]: https://abundance.zulipchat.com/
