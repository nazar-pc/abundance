# Contributing

Thanks for considering contributing to Abundance!

This is an early-stage research project, so things move fast and change shape often. Read the [book] for architecture
details, the [website] for updates, and join [Zulip chat] for questions and discussion.

[book]: https://abundance.build/book

[website]: https://abundance.build/

[Zulip chat]: https://abundance.zulipchat.com/

Maintainer time is the scarce resource here, so the process is optimized for reviewers rather than for contributor
convenience: code is written once but read many times. Everything below follows from that.

## Issues and questions

If you are not sure whether something is a bug, ask in [Zulip chat] first, a GitHub issue can always be created later.
Search for existing issues before creating a new one, but don't hijack threads that look similar, they often have a
different root cause.

A useful bug report has four things: environment and the exact command used, expected result, what happened instead,
and relevant output verbatim. As much as it takes to reproduce it, no rambling.

For improvements and feature requests, consider discussing the idea in [Zulip chat] first, then describe concisely what
the change looks like from the user's point of view.

## Before you push

The toolchain is pinned in `rust-toolchain.toml`, do not override it. There is no stable Rust support and no MSRV,
unstable features are used deliberately except in a few crates published on crates.io that already compile on stable.

Nightly features are used where stable Rust has no ergonomic or fast enough equivalent: `const` evaluation (impossible
otherwise, and there are a lot of arrays and compile-time sized data structures here), standard library adapters and
helpers that would otherwise have to be reimplemented, and nicer APIs around uninitialized memory. Anything not
particularly controversial or on its way out beats reimplementing it or checking at runtime what can be proven at
compile time.

Baseline that must be executed and produce zero warnings:

```bash
cargo fmt
cargo clippy --all-targets # Also how you check that things compile, `cargo build`/`cargo check` are not needed
cargo nextest run # This is faster, `cargo test` also works
```

CI additionally runs Clippy and tests for each crate and feature combination individually, under Miri, for RISC-V
targets, and builds crates with a `no-panic` feature enabled. Running all of that locally is not expected, the workflows
are your guide if you do, and you can always trigger CI on a branch in your fork before opening a PR.

Rustdoc is built with broken intra-doc links denied, so make sure links in documentation actually resolve.

## Code style

This is high-reliability and high-performance code, and most of the rules below follow from that: absence of panics
proven at compile time when possible, `unsafe` used in a targeted way and with rigorous proofs, allocations avoided, and
APIs designed such that invalid states are unrepresentable and invalid usage doesn't compile. Hence all the fixed-size
arrays, including heap-allocated ones like `Box<[[u8; OUT_LEN]; N]>`, rather than slices and `Vec`s of a dynamic size.
Higher-level and less sensitive code is not held to the same standard, use judgment. Otherwise, mimic the existing code
and prefer obvious code to clever code.

### Correctness

* `.unwrap()` is forbidden outside of tests and benchmarks, use `.expect()` instead. The message is a proof for the
  reviewer that it will never panic (or, rarely, why panicking is the preferred outcome), ending with `; qed`, as in
  `NonZeroU8::new(1).expect("Not zero; qed")`. It must follow from local context for a standalone function, or from data
  structure invariants for an internal method, otherwise return `Result<T, E>`.
* Avoid indexing with `[]`, prefer `.get()` and friends with explicit handling of the missing case.
* Use explicit checked, wrapping, or saturating math, especially in consensus-critical code. Don't reach for saturating
  math just to make things compile: if business logic doesn't expect an overflow, use checked math with `.expect()` or
  return an error.
* Avoid `as` for conversions, it truncates silently. Prefer `T::from(value)` over `value.into()` so the resulting type
  is clear right away, unless naming the type would require a new dependency. Same for `T::try_from(value)`, with the
  error returned (or `expect()`ed if it can't fail). Use `NonZero*` types where zero is not a valid value.
* Clippy requires a `// SAFETY:` comment on every `unsafe` block and a `# Safety` section on public `unsafe` functions,
  which internal ones get here too. What it can't check is the comment itself: a proof of soundness that can't be broken
  through the public API from safe Rust, referring to the invariant rather than to what the code does:
  ```rust
  #[inline(always)]
  const fn strong_count_ref(&self) -> &AtomicU32 {
      // SAFETY: The first bytes are allocated for `strong_count`, which is a correctly aligned
      // copy type initialized in the constructor
      unsafe { self.buffer.as_ptr().cast::<AtomicU32>().as_ref_unchecked() }
  }
  ```
  If you reached for `unsafe` to make the compiler stop complaining, you are probably doing it wrong, ask for help
  instead.
* The compiler verifies the absence of panics where feasible. Crates that can offer this guarantee have an optional
  `no-panic` feature and annotate their public API accordingly:
  ```rust
  /// Get the root of Merkle Tree
  #[inline]
  #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
  pub fn root(&self) -> [u8; OUT_LEN] {
      *self
          .tree
          .last()
          .or(self.leaves.last())
          .expect("There is always at least one leaf hash; qed")
  }
  ```
  Keep this in mind when editing such crates, an innocent-looking change can introduce a panicking branch and fail the
  build.
* Leave a `TODO` explaining what is missing when code is incomplete or a known issue is not handled, and create an issue
  for big things. When a workaround is needed because of an upstream bug or a missing feature, report it upstream and
  link that issue in a `TODO`.

### Lints

Clippy is configured in `[workspace.lints]`, inherited by every crate, with `pedantic` and `restriction` enabled
wholesale and individual lints allowed back with a comment explaining why. It will demand `#[expect(...)]` over
`#[allow(...)]` and a `reason`; what it can't demand is that the reason explains the cause and links the upstream issue
where there is one:

```rust
#[expect(
    clippy::cast_ptr_alignment,
    reason = "False-positive, see https://github.com/rust-lang/rust-clippy/issues/17636"
)]
```

Some rules above are stricter than what Clippy currently enforces: `unwrap_used`, `as_conversions` and
`indexing_slicing` are allowed at the workspace level for practical reasons explained in the comments there. Review
enforces those instead.

### Readability and structure

* Crates are named `ab-<area>-<subject>` and live under `crates/{shared,node,farmer,execution,contracts}` according to
  who uses them. Proc-macro crates are split into a thin `ab-*-macros` and an `ab-*-macros-impl` holding the logic, so
  it can be tested and reused.
* Modules use the `foo.rs` + `foo/` layout, `mod.rs` is not used anywhere.
* Tests of internal invariants, especially those needing test-only interfaces, live in a sibling `foo/tests.rs` rather
  than inline in the implementation file. For a library with a clear public API prefer `tests/`: the API is exercised as
  users see it, and each file compiles into a separate binary, which helps concurrency testing in CI and under Miri.
* A struct definition and its implementation belong in the same file, ideally with nothing in between. Trait impls come
  first, inherent impls after that.
* Prefer longer variable names, 1-3 character names are usually a bad choice. Exceptions like `id` in an entity or `i`
  in a simple loop are fine, though iterator chains often express the same thing without an explicit index.
* Take advantage of type inference to remove noise, but keep the code readable without an IDE displaying inferred types.
  In particular, prefer `let collection = iter.collect::<Vec<_>>()` over `let collection: Vec<Type> = iter.collect()`:
  same meaning, better formatting, and the type stays where it is created.
* A comment that is a single sentence has no `.` at the end, a comment with several sentences has one after each of
  them.
* Dependencies in `Cargo.toml` are kept sorted, adding one out of order introduces entropy and irritates maintainers.
* Every file ends with exactly one newline, and no line ends with whitespace, in code, Markdown, configs, and everything
  else. Configure your editor to do it automatically.

### Types and APIs

* Most crates, especially low-level primitives, are `no_std` by default, but may opt into `alloc` or `std` if really
  needed.
* Primitives are wrapped in newtypes, so a block number can't be passed where a shard index is expected, with arithmetic
  and conversions implemented deliberately rather than inherited from the underlying type.
* Library crates define precise error types with `thiserror`, typically one enum per fallible operation, linked from
  documentation. `anyhow` is only for binaries:
  ```rust
  /// Error for [`derive_consensus_parameters()`]
  #[derive(Debug, thiserror::Error)]
  pub enum DeriveConsensusParametersError {
      /// Failed to get ancestor header
      #[error("Failed to get ancestor header")]
      GetAncestorHeader,
  }
  ```
* Layout and size matter for data structures that are performance-sensitive, persisted or sent over the network, hence
  `#[repr(C)]`/`#[repr(transparent)]` and explicitly sized fields there. Higher-level types that are neither hot nor
  serialized often don't need this.
* Low-level APIs, core primitives in particular, should be `const fn` where possible, some of them are used at compile
  time. Not a requirement for higher-level logic, but writing `const` code forces you to avoid allocations, which is
  good for performance and for absence of panics anyway.
* Simple and proxy methods in low-level crates get `#[inline(always)]` right away, for performance but also for absence
  of panics: without inlining the compiler often can't see enough to prove a panicking branch unreachable.
* `async-trait` is used where `dyn Trait` is needed or an external API requires it, not by default.
* Logging uses `tracing` with structured fields instead of string interpolation, `info` for lifecycle events (mostly
  reserved to binaries, not libraries), `warn` for recoverable surprises, `debug`/`trace` for internals:
  ```rust
  warn!(%error, "Failed to send block importing notification");
  ```
* Public items are documented, and crate-level `//!` documentation explains what the crate is for and how the pieces fit
  together, not just names it.
* Performance-sensitive crates carry Criterion benchmarks in `benches/`, extend them when changing a hot path.

### API changes

New code should look like a natural extension of the existing architecture, and existing APIs usually have a good reason
for being the way they are, so find that reason before changing them. When an API genuinely stops fitting, don't bolt
the new use case onto it: think hard about what the architecture should look like, do the refactoring first as a
separate commit or even a separate pull request, and land the intended change on top, where it now fits naturally. It is
normal to discover this in the middle of working on a feature, which is one of the things rebasing is for: write the
refactoring and rebase the in-progress changes onto it.

## Commits

Commits should tell a logical step-by-step story and each be individually meaningful. A reviewer should be able to go
through them one by one, with non-trivial changes it is often tough to review the final diff.

Write commit messages in the present tense, describing what the change does, with identifiers in backticks:

> * Reject AMOs whose misaligned access crosses the atomicity granule
> * Store B-type branch offsets as `i16` rather than `I24`
> * Extract standalone per-instruction execution functions as preparation for future reuse
> * Install prebuilt RISC-V toolchain, upstream repos are too unreliable to be used lately

The first two say what changed rather than which files were touched, the third justifies a refactoring commit, and the
last carries a reason that the diff doesn't show.

Avoid garbage commit names like "fix", "wip", "🤦 x 2", "......", "AHHHHHHHHH" (those are real examples), such commits
should usually be squashed into another commit instead. See [Write Better Commits, Build Better Projects] for more.

[Write Better Commits, Build Better Projects]: https://github.blog/2022-06-30-write-better-commits-build-better-projects/

More specifically:

* Use squashing, rebasing, and reordering freely during development; there is great tooling for this. If an API
  introduced earlier in the branch turns out to be wrong, amend the original commit rather than fixing it up later,
  otherwise a reviewer going through commits will comment on something that is already fixed or gone in the end.
* Different kinds of changes belong in different commits, if not different pull requests. Move something in one commit
  and change it in another, so the reviewer can skim the move and focus on the change. The same applies to renames:
  renaming and rewriting a file at once makes Git treat it as a delete plus an add, which breaks history tracking.
* If refactoring is needed for a feature, consider extracting the refactoring commits into a separate branch, submitting
  that for review with the rationale, and rebasing the feature branch on top of it.
* Keep the number of commits reasonable: 70 commits for 100 lines of changes is bad, and so is one commit changing
  thousands of lines all over the place.
* For automated changes like a mass rename or a formatting pass, put the command in the commit message, so the reviewer
  can run it and check that it produces the same diff.
* Push regularly, but not necessarily every commit, since it may occupy CI time.

## Pull requests

Many people are subscribed to this repository, and every change to a pull request notifies all of them. Minimizing that
noise is the reason behind most of the rules below.

* Do a full self-review before submitting. Use force-pushes at this stage to get the history into the shape you would
  want to review yourself.
* Make sure your code builds and tests pass locally before submitting, rather than discovering it from CI afterward. Add
  test cases where applicable.
* Write a description that gives the reviewer useful context: what the change does, why, and what the alternatives were.
  A trivial typo fix needs none. Don't repeat what the reviewer can see faster in the code, LLMs are notoriously guilty
  of this. Link related issues and pull requests, using [GitHub linking keywords] where applicable.
* Open a pull request only when it is ready for review. Drafts that are updated frequently are a major source of
  distraction, and WIP changes can be shared as a branch diff or a pull request in your own fork.
* Once a pull request exists, try to avoid force pushes, they may make the maintainer re-review from scratch and can
  make old commits non-compilable, which hurts future debugging. Prefer meaningful commits on top. Exceptions are
  trivial typo or rename fixes (don't rebase at the same time, the diff will explode) and a requested refactoring that
  reshapes the pull request completely. Judge by what makes review easier rather than harder.
* To debug an unknown CI issue, push to a separate throwaway branch instead of updating the pull request repeatedly.
  Workflows can be triggered on branches explicitly, including in forks.
* Address review comments in a few commits pushed at once, not one by one. Resolve only the comments that are trivially
  and completely addressed, leave the rest for the person who wrote them, and re-request review when you are done.
* When leaving more than one comment, always post them as a review from the "Files changed" tab and submit it all at
  once.
* A large effort is generally better as a sequence of small pull requests than as one big one. Dependent ones can
  target each other's branches, GitHub retargets them automatically as they get merged, though this requires access to
  create branches in the repository.
* Pull requests are merged without squashing in most cases, which keeps moves and refactorings separate and helps with
  bisection later. Only trivial or otherwise unreviewable changes are squashed, and the expectation is that the author
  does that before opening the PR in the first place.

[GitHub linking keywords]: https://docs.github.com/en/issues/tracking-your-work-with-issues/linking-a-pull-request-to-an-issue
