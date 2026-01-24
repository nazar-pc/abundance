---
title: Contracts are actually running
date: 2025-02-07
draft: false
description: Native execution environment is finally functional
tags: [ status-update ]
authors: [ nazar-pc ]
---

After a lot of refactoring and preparation, native execution environment is finally functional and can be used for
purposes like writing tests and debugging.

<!--more-->

While the previous update mentioned metadata decoding for methods, turns out full metadata decoding was necessary for
native execution environment and was implemented in [PR 45] with some fixes and optimizations landing in [PR 46] shortly
afterward. One important change there was avoiding `#[slot]` and `#[tmp]` type repetition in metadata, instead it is
stored in the main contract metadata right next to the state metadata, which is part of the reason why full metadata
decoding was needed at this stage.

[PR 45]: https://github.com/nazar-pc/abundance/pull/45

[PR 46]: https://github.com/nazar-pc/abundance/pull/46

After that, [PR 47] made one of the last changes to FFI interface, moving size and capacity as pointers next to the data
pointer in both `InternalArgs` and `ExternalArgs` for easier processing by the host. It really made things a lot simpler
to handle than before and since they are pointers now, there are no issues with alignment of fields that have different
types in those data structures.

[PR 47]: https://github.com/nazar-pc/abundance/pull/47

With that, [PR 50] finally introduced a native execution environment alongside some basic tests of the example contract.
[PR 55] further simplified things a bit, and as of right now, a contract test looks [something like this]. As you can
see, it supports deploying system contracts first, deploying user contracts, various calls into contracts both from
outside execution environment and cross-contract calls, both directly and through trait interfaces like `Fungible` trait
for pseudo-token (implemented by example contract). A massive step forward overall!

[PR 50]: https://github.com/nazar-pc/abundance/pull/50

[PR 55]: https://github.com/nazar-pc/abundance/pull/55

[something like this]: https://github.com/nazar-pc/abundance/blob/f240e3c7cca20439e92d177fa6529fef61e557c4/crates/contracts/ab-contract-example/tests/basic.rs

I mentioned `InternalArgs` and `ExternalArgs` before, but what are they? Great question indeed! With FFI interface being
a bit more stable now, I have expanded [contract macro documentation] with details about what code it actually generates
and how it is supposed to be used in [PR 52]. I know it is a lot of text, but it should still be a bit easier to
comprehend than trying to infer what it does and why from the macros source code (even though I tried to document it as
well).

[contract macro documentation]: /rust-docs/ab_contracts_macros/attr.contract.html

[PR 52]: https://github.com/nazar-pc/abundance/pull/52

In the process of doing that, I was more and more annoyed by the fact that there are raw pointers in Rust that can
express whether the pointer can be used for writes (`*mut T` vs `*const T`) and there is a pointer that is guaranteed to
be not `null` ([`NonNull`]), but there is no `NonNullConst` and `NonNullMut` pair or similar. Discussion
on [Rust Internals] indicates I'm not alone, so hopefully things will improve in the future and make FFI interfaces even
better.

[`NonNull`]: https://doc.rust-lang.org/stable/core/ptr/struct.NonNull.html

[Rust Internals]: https://internals.rust-lang.org/t/two-flavors-of-nonnull-again/22321

Outside of code changes, there was one researcher interview and there are a few more leads that will hopefully turn into
more interviews later, thanks Michelle! Two developer interviews are also planned for next week; I'll share how those
went hopefully in the next update.

## Upcoming plans

While the native execution environment is there and even supports recursive calls into other contracts, it unfortunately
doesn't perform a couple of important checks. First it doesn't reject `#[update]` or `#[init]` calls from `#[view]`
methods, but it really should despite generated extension traits making it impossible to compile when using high-level
APIs. Second, conflicting state modifications in recursive calls are not prohibited, which are trivial to do in
contracts to the first issue. I'll work on fixing these next.

And interviews I'm sure will lead to some interesting feedback to reflect on. If you know someone with smart contact or
blockchain development experience that would be good to talk to, let me know. 
