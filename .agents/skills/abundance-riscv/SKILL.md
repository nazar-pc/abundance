---
name: abundance-riscv
description: Repository-specific knowledge for RISC-V work in this repository specifically (not RISC-V in general) — macro behavior, crate layout, prelude/import conventions, design principles, testing expectations, the ACT4 conformance test runner, and where to measure performance. Use this whenever working on anything involving RISC-V (primitives, interpreter, execution, codegen macros, conformance tests, or benchmarking), even if the user doesn't name these crates explicitly. Should be consulted before editing any `ab-riscv-*` crate or reasoning about how code in those crates behaves. Do not apply to RISC-V questions unrelated to this repository.
---

# Abundance RISC-V

Domain knowledge for the RISC-V-related crates in this repository. Consult this before editing or reasoning about code
in any `ab-riscv-*` crate.

## Macros

Code-generating macros used throughout this code are defined in `crates/execution/ab-riscv-macros-impl/src/lib.rs`.
These macros modify code in ways that may not be obvious from reading the call site alone — check this file before
assuming what a macro-annotated item actually does or generates.

## Crate layout

### `crates/execution/ab-riscv-primitives`

Instruction decoding and general RISC-V primitives. There are a lot of them, so essentially everything is re-exported
from the crate's `prelude`.

- Outside the crate: always use the prelude.
- Inside the crate: import items by their full local path, not through the prelude.

### `crates/execution/ab-riscv-interpreter`

The RISC-V interpreter implementation. It also has a `prelude`, which re-exports everything *except* the basic
implementations in the `basic` module.

- Outside the crate: use the prelude, plus explicit imports from `basic` when those are needed.
- Inside the crate: full local paths, same as primitives.

**Convention**: helpers intended for downstream/generated code live in `*_helpers` modules and are included in the
prelude, so generated code doesn't need hundreds of explicit imports. This convention only applies to what's exposed to
downstream users — code inside the crate itself still uses explicit full paths.

### Design principles (both crates)

Before implementing new instructions or making non-trivial changes, read the documentation at the top of `lib.rs` in
whichever crate you're touching — it explains the design decisions behind the code. The code in both crates is designed
to be:

- Spec-compliant and strict (Ssstrict).
- Completely free of panics.
- Free of heap allocations.
- As idiomatic Rust as possible.

Keep these properties in mind for any change — don't introduce a panic path, an allocation, or a spec deviation to make
something simpler.

### Testing

Both `ab-riscv-primitives` and `ab-riscv-interpreter` have extensive existing test suites. Any fix should include
regression tests and/or update existing tests accordingly — this is a stricter, crate-specific version of the general
"add regression tests" convention, since test infrastructure is already well established here.

### `crates/execution/ab-riscv-act4-runner`

Runner for the RISC-V Architectural Certification Tests (ACT4, see
[riscv/riscv-arch-test](https://github.com/riscv/riscv-arch-test)). DUTs (devices under test) for various virtual cores
also live here.

The test ELFs are not committed to the repo, CI generates them with the ACT4 Docker image (the `act4` job in
`.github/workflows/rust.yml`), and the crate's `README.md` shows how to do the same locally. Where Docker or the image
isn't available, verify that the code compiles rather than trying to execute the test suite, and say so.

Core definitions in `res/abundance` describe what the interpreter actually implements, see the README there for why each
core enables what it does. When ACT4, Sail or the unified DB require something a core doesn't declare (vector tests
needing M, SsstrictSm needing Zca, vector privileged suites needing full V), check the specification first: it may well
be a bug upstream rather than something to implement in the interpreter. Some requirements are real though (HPM
counter CSRs are expected to exist, even if read-only zero), so look it up rather than assuming either way.
Check whether it was already reported, prepare a fix or a bug report for upstream (following that repository's
contribution guidelines) and keep any workaround here in its own commit with a `TODO` linking the upstream issue, so it
can be reverted cleanly once fixed.

Pitfalls when running ACT4 locally:

- With a persistent work directory, ACT4 keeps generator stamps in `work/stamps` while generated test sources live
  inside the container, so later runs silently skip test generation. Remove `work/stamps` after changing core
  configurations or the ACT4 version.
- ACT4 doesn't delete ELFs of tests that are no longer selected, and ELFs built with a different Sail version are
  outdated. Remove old work directories rather than reusing them across versions.
- Sail's configuration has a public JSON schema, validate `sail.json` against it when updating Sail.

## Performance work

If a task is about performance, there are two relevant benchmark crates:

- `crates/execution/ab-riscv-coremark-runner` — classic CoreMark, run through the threaded path. `COREMARK_ITERATIONS`
  is fixed at build time (`build.rs` explains why the default is useless for comparisons), the RISC-V toolchain is
  `riscv64-unknown-elf-gcc` (override with `RISCV_CC`, use the `build-elf-required` feature to make a missing toolchain
  a build error). Below about ten seconds of run time CoreMark prints `ERROR! Must execute for at least 10 secs` and
  `Errors detected`, which is its reporting rule and not a failure; check the per-algorithm CRCs instead (`crclist`
  `0xe714`, `crcmatrix` `0x1fd7`, `crcstate` `0x8e3a`).
- `crates/execution/ab-riscv-benchmarks` — Criterion benchmarks of a guest contract (BLAKE3 hashing, ed25519 signature
  verification), natively and through the interpreter as `interpreter/loop/lazy`, `interpreter/loop/eager` and
  `interpreter/threaded/eager`. The lazy one is a control: a change to the eager or threaded path that moves it is
  measuring layout, not the change.

Eager (pre-decoded) and threaded execution are the primary optimization targets. Other performance improvements matter
too, but are secondary to those two.

### Measuring

The reference machine is the maintainer's Zen 4 (Threadripper 7970X), with the benchmark pinned to a core. Cloud and
sandbox machines change CPU between sessions (and sometimes within one), and timings there have disagreed with Zen 4 in
direction, not just magnitude, so they can't settle a performance question. They can settle structural ones: type
sizes, whether something is returned in registers or through a hidden out-pointer, instruction counts and frames in the
release disassembly of a handler, correctness. Do that part yourself and leave timing claims for the maintainer to
confirm, saying which machine any number came from.

When timing anything: check `lscpu` first, compare only within a single run, interleave the A and B builds round-robin
over several rounds rather than running all of A and then all of B, and report best-of together with the run-to-run
spread, an effect narrower than the spread is not a result.

Layout noise is larger than most effects being chased. Moving hot code relative to cache lines changes results
substantially, and a benchmark that shares no code with a change can still move, which is evidence about layout rather
than about the change. Source-level alignment does not fix this (aligning one function relays out everything else),
PGO does. So prefer CoreMark (one program in one binary) over the Criterion benchmarks (which link the interpreter next
to BLAKE3, ed25519 and Criterion itself) for small effects, and confirm suspicious results with PGO builds of both
sides. PGO only helps when hot functions are small: it made the old monolithic `match` loop worse.

The hot path sits on a register-pressure cliff. Keeping one more value live across it, for example caching a bound in
a field instead of re-reading it, can cost far more than the load it saves, and whether a change lands on the right
side of the cliff depends on the codegen configuration (`-C target-cpu` included), so don't extrapolate from one
configuration to another.

### What not to propose

The threaded dispatch shape (safe generic handlers chained with `become`, register file as a type parameter, results
describing control flow instead of handlers moving the program counter) was chosen by measuring alternatives, see
[references/dispatch-experiments.md](references/dispatch-experiments.md) for what was tried, why it lost and which open
ideas remain. The maintainer's blog posts in `website/main/content/blog/2026/` (the ones about making the interpreter
faster) record many more attempts that failed and why, read them before proposing a non-trivial optimization. In short:

- Bounds checks in `VirtualMemory` implementations are part of the correctness contract (this runs blockchain
  consensus), they are not overhead to optimize away.
- Profiles of the threaded path show it front-end bound with both caches effectively always hitting, so software
  prefetch hints only spend the scarce resource.
- Register file traffic is a real cost because it causes store-to-load forwarding stalls. That is why `BasicRegisters`
  handles `x0` differently for the `match` loop and for threaded execution, and why simply dropping one of its `x0`
  checks was slower. Promoting hot guest registers into locals for LLVM to keep in host registers made things much
  slower too, a function as big as the `match` loop defeats SROA.
- Superinstructions for a handful of hand-picked pairs don't pay: executed adjacent pairs are spread flat, with no small
  hot set. Fusion is still expected to be the next big win, but as a general mechanism rather than special cases.
