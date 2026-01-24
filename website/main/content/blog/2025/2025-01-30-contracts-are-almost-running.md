---
title: Contracts are almost running
date: 2025-01-30
draft: false
description: Preparation for running contracts in native execution environment
tags: [ status-update ]
authors: [ nazar-pc ]
---


Last week was busy with refactoring with the primary goal of being able to run contracts in test execution environment.
The environment is not quite ready yet, but a lot of progress was done, and it'll hopefully be ready next week.

<!--more-->

The plan is to have several execution environments. Naturally, the blockchain environment will run a VM with gas
metering, etc. But it is less convenient to debug, which is why test/native execution environment will be available as
well that can run things exactly the same way, but with access to usual debugging and other tools. The data flow will
still happen through the same FFI functions as guest environment in a VM, which means there will be as few actual
code path differences as possible.

To improve certainly in code, especially because there is a decent amount of `unsafe`, some of which is auto-generated,
safety is important and [Miri] is a great tool for this purpose. Since execution environment is generic and doesn't
really know types it deals with at compile time, I initiated a [discussion on Rust forum] to make sure Strict Provenance
is possible even in this unusual situation.

[Miri]: https://github.com/rust-lang/miri

[discussion on Rust forum]: https://users.rust-lang.org/t/provenance-when-reading-pointers-from-erased-type/124771?u=nazar-pc

Contract and method metadata in particular was introduced a while ago to describe to the host and to avoid higher-level
tools what contract contains in terms of its FFI surface, but it was far from complete.

[PR 35] addressed function fingerprint only having a non-functional stub implementation. Function signature is somewhat
similar to function selector in EVM, except it uses cryptographically secure hashing function (due to the need of being
able to compute in `const` function [`const-sha1`] crate is used, but I also opened [a PR] a few weeks ago to make
`blake3` work in `const` functions too, but it is not merged yet) and is supposed to uniquely represent signature of a
method when making external method calls (call will fail in case of fingerprint mismatch).

[PR 35]: https://github.com/nazar-pc/abundance/pull/35

[`const-sha1`]: https://github.com/rylev/const-sha1

[a PR]: https://github.com/BLAKE3-team/BLAKE3/pull/439

Metadata compaction was also implemented as part of [PR 35] that strips data structures of unnecessary details. This
allows implementations to change argument, field and data structure names as long as the shape of the data is exactly
the same, without affecting fingerprint.

Getting closer to execution environment implementation and striving to great developer experience, storing of function
pointers was introduced in [PR 40] (and improved in [PR 42]), which implements a global registry of all methods provided
by all contracts that are being linked into the binary using [`linkme`] crate. This means that no explicit actions are
necessary for developers beyond adding crate to dependencies, which they would have to do anyway to be able to deploy a
contracts, access its helper methods, etc. I do not like that it is so implicit too much, but I think the context and
usability win justifies it.

[PR 40]: https://github.com/nazar-pc/abundance/pull/40

[PR 42]: https://github.com/nazar-pc/abundance/pull/42

[`linkme`]: https://github.com/dtolnay/linkme

To be able to dynamically and "manually" construct data structures of correct shape, it is necessary to process metadata
in various ways, which is tedious and error-prone. [PR 43] introduced utilities to decode method metadata and return
something that is more convenient to use for creating and reading internal (host → guest) and external (guest → host)
data structures properly. It also verifies metadata as it processes it, rejecting invalid contents (which is another
"free" test case for metadata generation).

[PR 43]: https://github.com/nazar-pc/abundance/pull/43

With those and some more minor improvements and refactoring (which as always, you're free to check out in individual
PRs), native execution is almost here. A bit more work still remains that will have to wait until the next update before
contracts are actually running for real.

## Upcoming plans

Just like last time, the plan is to work on execution environment to be able to run contracts together and have a better
understanding of what it feels like as a whole. Once that is done, I'll be doing developer interviews collecting
unfiltered feedback from other developers about the system before moving much further. It is important to understand if
anyone actually wants something like this or not before investing too much effort into it.

Additionally, interviews will start with potential candidates that might help with sharding design research, which I
could use a lot of help with, especially with math.
