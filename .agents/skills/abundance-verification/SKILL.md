---
name: abundance-verification
description: >-
  How to verify changes in the abundance repository proportionately and what its CI actually runs (Clippy and tests
  per crate and feature, Miri on several OSes and RISC-V targets, riscv32 as the 32-bit target, the custom guest target,
  `no-panic` builds, stable `rust-version` crates, rustdoc). Use whenever checking, testing or changing CI in this
  repository, and before deciding which checks to run for a change.
---

# Verifying changes in abundance

`CONTRIBUTING.md` ("Before you push") has the baseline: `cargo fmt`, `cargo clippy --all-targets` and
`cargo nextest run`, zero warnings. Everything else CI does is defined in `.github/workflows/rust.yml` (rustdoc is in
`gh-pages.yml`). Read the workflow when you need exact flags, it is commented, and treat this page as a map of it plus
the reasoning that isn't written there.

## Proportionality

Every commit must compile and pass tests on its own (`CONTRIBUTING.md`, "Commits"), and the baseline from "Before you
push" (`cargo fmt`, `cargo clippy --all-targets`, `cargo nextest run`) must be clean. Running the baseline for the
crates a commit touches and the crates depending on them covers everything a change can affect, the rest of the
workspace is unchanged, so that is what to run per commit. Run it at workspace scope when a change affects every crate,
like workspace `Cargo.toml`, lints, the toolchain or a dependency update in `Cargo.lock`. The heavy, slow CI slices
(Miri, riscv32, the guest target, `no-panic`, per-feature runs, rustdoc) run once on the tip, and only those the change
can actually affect. If one fails, find the commit that caused it and fix it there, rather than re-running everything on
every commit.

Pick slices by what changed:

- `unsafe`, raw pointers, layout or alignment code: Miri for the affected crate.
- `usize`/pointer-width arithmetic, atomics, anything sized by the platform: riscv32 (Clippy, and Miri if tests exist).
- Crates under `crates/shared/` that must work without `std`, or contract crates: the guest target build.
- A crate with a `no-panic` feature, especially code under an annotation: the `no-panic` build of that crate.
- A crate declaring `rust-version`: its tests on that stable version.
- Doc comments with links or examples: rustdoc and `cargo test --doc` (nextest doesn't run doc tests).
- A rebase onto changes that don't overlap with yours, a reworded comment, a test gating change: nothing new to run.

When the maintainer says they will run CI themselves, don't duplicate it locally.

## What usually fails only in CI

riscv32, Windows, macOS and `no-panic` failures are the ones that tend to show up only in CI. Approximate the slices
relevant to the change locally (commands below) before handing a branch over, especially Miri on a foreign target and
the `no-panic` build, or trigger CI on a branch in your fork.

## What CI runs and how to reproduce a slice

Environment shared by all jobs: `CARGO_BUILD_WARNINGS=deny` (set it locally to get the same failure on warnings) and
`MIRIFLAGS=-Zmiri-disable-isolation` (tests touch the file system). `-Zgitoxide -Zgit` in the workflow only speed up
fetching and can be dropped locally.

`cargo-clippy` runs on every platform and target with three types: the workspace `together`, each crate
`individually`, and `features`, where every crate under `crates/{execution,farmer,node,shared}` having one of the
features in `INDIVIDUAL_FEATURES` is checked with just that feature, plus `guest` in contract crates and a couple of
named features. To reproduce one cell:

```bash
cargo clippy --all-targets --package ab-foo --features ab-foo/alloc
```

`cargo-test` and `cargo-test-slow` share a platform matrix where each entry is either native or under Miri
(`together` lives in the `-slow` job only so it starts first). Miri runs natively on Linux, macOS and Windows runners,
and on foreign targets `riscv64a23-unknown-linux-gnu`, `x86_64-pc-windows-msvc` and `riscv32gc-unknown-linux-gnu`.
Under Miri CI uses `cargo miri test` (nextest is slower there), and splits large test binaries into separate
invocations according to `.github/miri-test-partitioning.txt`:

```bash
MIRIFLAGS=-Zmiri-disable-isolation cargo miri test --package ab-foo
MIRIFLAGS=-Zmiri-disable-isolation cargo miri test --package ab-foo -- test_name
```

riscv32 has no prebuilt standard library, so the workflow adds `-Zbuild-std`, and compiles C code from build scripts
with Clang without target headers (the `RISC-V support` step explains why that is enough). The same slice locally:

```bash
TARGET_CC=clang TARGET_CFLAGS=-ffreestanding MIRIFLAGS=-Zmiri-disable-isolation \
  cargo miri test -Zbuild-std --target riscv32gc-unknown-linux-gnu --package ab-foo
TARGET_CC=clang TARGET_CFLAGS=-ffreestanding \
  cargo clippy -Zbuild-std --target riscv32gc-unknown-linux-gnu --all-targets --package ab-foo
```

The guest target is the custom JSON spec in `crates/contracts/core/ab-contracts-tooling/src/`
(`ABUNDANCE_RISCV64_TARGET` in the workflow). It has no OS, so only crates under `crates/shared/` minus the
`skip_crates` list are compiled, Clippy uses `--lib`, and tests are not run. Instead of tests, crates are built with
`cargo build`, not `cargo check`: many invariants here are `const` assertions in generic code that only fire during
monomorphization, which `check`/`clippy` never reach. `-Clink-dead-code` is applied to workspace crates only, so unused
and `#[inline]` items get code generated too; it is deliberately not used globally or for other targets:

```bash
TARGET_CC=clang cargo build -Zjson-target-spec \
  --target crates/contracts/core/ab-contracts-tooling/src/riscv64-unknown-none-abundance.json \
  -Zbuild-std=core,alloc -Zprofile-rustflags \
  --config 'profile.dev.rustflags=["-Clink-dead-code"]' --config 'profile.dev.package."*".rustflags=[]' \
  --package ab-foo
```

The `contracts` job additionally builds example and system contracts with `cargo run --bin cargo-ab-contract -- build
--package <crate>` (also with `--profile contract`).

`no-panic` builds every crate having that feature, in release mode with LTO and a high inlining threshold so the
compiler can prove more, on all OSes and `riscv64a23`. Crates are built one by one because building them together
doesn't prove the same things (see the `TODO` in the job). Use a separate target directory, the flags invalidate the
regular build cache:

```bash
CARGO_TARGET_DIR=target/no-panic \
RUSTFLAGS="-Cllvm-args=--inline-threshold=10000 -C embed-bitcode -C lto -Z dylib-lto -C target-cpu=native" \
  cargo build --release --all-targets --features no-panic --package ab-foo
```

Crates that also have an `alloc` feature are built a second time with it, which covers more of their API, so add
`--features ab-foo/alloc` when a change is behind it. Proofs only mean something in this optimized build: Clippy and
`cargo check` never link, so they can't see what `no_panic` checks, and debug builds keep panicking branches that
optimizations remove, so don't use the `no-panic` feature with them.

`cargo-test-rust-version` runs `cargo +<rust-version> nextest run` for each crate that declares `rust-version`, these
are the crates meant to work on stable. Only tests are run there, not Clippy.

Rustdoc (`gh-pages.yml`) is built with `--all-features` and broken or private intra-doc links denied:

```bash
RUSTDOCFLAGS="-D rustdoc::broken-intra-doc-links -D rustdoc::private_intra_doc_links" \
  cargo doc --no-deps --lib --all-features --package ab-foo
```

The `act4` job runs RISC-V conformance tests, see the `abundance-riscv` skill for that.

## Tests on CI platforms

The Miri policy, test cost and 32-bit coverage are in the "Testing" section of `CONTRIBUTING.md`. In addition:

- When Miri lacks support on a particular OS or architecture, gate narrowly with a `TODO` linking the Miri issue, see
  `crates/farmer/ab-farmer/src/utils/tests.rs` (`cfg(not(all(miri, target_os = "windows")))` and macOS) and
  `crates/execution/ab-riscv-interpreter/src/rv64/zk/zkn/zknd/tests.rs` (aarch64 intrinsics). Where a test needs to
  drive a future without a runtime, `futures::executor::block_on` is what the client database tests use.
- riscv32 has no 64-bit atomics (`max-atomic-width` is 32), so code and tests that run there use `AtomicU32` or
  `AtomicUsize`.
- Windows runners have less memory, so large allocations in tests may need to be combined or skipped there.
- For large values like blocks, compare with `assert!(a == b, "...")` instead of `assert_eq!()`, the `Debug` output of a
  failed `assert_eq!()` can be gigabytes of logs (see `crates/shared/ab-archiving/tests/integration/reconstructor.rs`).

## When `no-panic` can't be proven

If the compiler can't prove a function doesn't panic and restructuring isn't reasonable (panics in a dependency like
`blake3` or `schnorrkel`, executor calls through `Env`), comment the attribute out with a `TODO` naming the cause, and
do the same for callers that become unprovable because of it. Crates with such gaps summarize them in a `TODO` next to
the `no-panic` feature in `Cargo.toml`, see `ab-system-contract-simple-wallet-base`. Don't comment it out to get around
a panic you introduced: an allocation that can grow, like `Vec::resize()`, is usually replaceable by a preallocated
buffer that returns an error when its capacity would be exceeded.

Proofs rely on inlining (see `CONTRIBUTING.md`). A helper kept out of line on purpose, to keep a large stack frame off
the hot path for example, prevents inlining only in normal builds with
`#[cfg_attr(not(feature = "no-panic"), inline(never))]`, as in `ab-merkle-tree`'s `sparse.rs`. Array `map()` can leave
code that cleans up a partially initialized output on unwinding, which keeps `no_panic` from proving the callers, so
fill arrays in a loop instead (see the parallel test hashers of `ab-merkle-tree`).

## Changing CI

CI complexity is already high and has to stay manageable, so changes are an extension of what exists rather than
something new next to it:

- Add a target as a matrix entry and reuse the existing selection logic, rather than adding a job or dumping extra
  commands into a loosely related job because it is convenient. Don't propose bespoke steps for a single crate or test.
- Select crates by data from `cargo metadata`: path (`/crates/shared/`), presence of a feature, `rust_version`. Don't
  hard-code lists of crates to include. Exclusions go into the `skip_crates` regex by crate name with a comment saying
  why, rather than conditional compilation in the crates; a crate depending on an excluded one needs its own entry.
- There is a single 32-bit target. If riscv32 stopped working, the whole CI would switch to another 32-bit target
  instead of mixing several.
- Reuse YAML anchors (`*checkout-step`, `*shared-test-steps`, `*parallel_worker`, ...) and shared `env` values like
  `INDIVIDUAL_FEATURES` instead of duplicating steps or lists.
- Keep `${{ }}` expressions readable: one condition per line for long ones, as in `skip_crates`; compute values in a
  shell variable and interpolate that; skip cases with `continue` in loops rather than growing an expression. Each
  non-obvious choice gets a comment explaining why, like the rest of the file.
- Actions are pinned to a commit hash with the version in a trailing comment.
