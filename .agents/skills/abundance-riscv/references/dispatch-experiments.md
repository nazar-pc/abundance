# Instruction dispatch: what was measured

Both execution paths are generated from the same instruction `match` arms: `BasicInterpreterState::execute()` drives the
`match`-based `ExecutableInstruction::execute()` in a loop, `ThreadedExecutableInstruction::execute_threaded()` runs
one handler per variant, each tail-calling (`become`) the next. The emitter is
`crates/execution/ab-riscv-macros/src/build/execution_impl/generate_threaded_fns.rs`, its output for any instruction
set is `$OUT_DIR/{EnumName}_execution_impl.rs` (errors reported against that file are emitter errors, not errors in
instruction sources), and `crates/execution/ab-riscv-interpreter/src/rv64/threaded_tests.rs` checks that both paths
agree. Refactorings of the emitter were gated on the release disassembly of the handlers staying identical, which is
a better check than a benchmark for changes that are not supposed to change codegen.

## Why handlers beat the `match`

The `match` loop is throughput-bound, not misprediction-bound, so the usual argument for threaded dispatch (better
branch target prediction) is not what matters here. What matters is that LLVM hoists operand decoding and source
register reads common to most arms in front of the jump table, so every instruction pays for operands it doesn't use,
and that the loop slows down as variants are added to the enum even if they are never executed. Handlers touch only
what their instruction names and don't care about the variant count.

## Measured and set aside

- Raw pointers and a context that owns guest memory, instead of the safe generic borrowed state: no gain.
- A call per instruction that returns to a loop: most of the win comes from never returning. An ordinary call in tail
  position compiles the same as `become` today, but only `become` guarantees it.
- Register files that handle `x0` by branching or with a conditional move: the unconditional write followed by
  re-zeroing (`BasicRegisters` with `ZEROSTORE`) was the fastest for threaded execution. This may need revisiting if
  the handler signature changes enough to alter register pressure.
- `extern "rust-preserve-none"` (unstable): with arguments ordered well it only ties the current setup, so it is an
  unstable feature for nothing today. It becomes interesting if the handler signature outgrows the six integer
  argument registers of `sysv64`, at which point an argument would be passed on the stack on every dispatch. If it is
  revisited on x86-64, mind the argument order: LLVM passes `preserve_none` arguments in R12, R13, R14, R15 first, and
  memory operands based on R12/R13 always cost an extra encoding byte, so the hottest pointers (fetcher position,
  register file) must not land there. Cold-first is right only while hot handlers call nothing; once they call out,
  R12-R15 are the registers that survive a call for free and the order flips. Not an issue on aarch64 or riscv64.
- `#[loop_match]`/`#[const_continue]`: `const_continue` needs a statically known target, while the next instruction
  is a run-time value, so all that remains is replicated dispatch sites. That targets indirect mispredicts, a small
  share of the run time here, and keeps the costs of one big function.
- `-C relocation-model=static`: removes the instruction that materializes the jump table address in every handler and
  is measurably faster, but costs ASLR and can't go into `RUSTFLAGS` (proc-macro crates must be position-independent),
  only `cargo rustc -- -C relocation-model=static -C link-arg=-no-pie` on the final binary.
- LLVM `-hot-cold-split` and `#[unsafe(link_section = ".text.unlikely")]` on cold continuations: no effect, lld merges
  the section into `.text` without reordering unless given `-z keep-text-section-prefix`.

## Things that mattered and are easy to undo by accident

Each of these was worth several percent and is documented on the item, read that before changing it: the fetcher being
`Copy`/`!needs_drop` (`InstructionFetcher`), `try_set_pc_relative()`/`failed_branch()` being separate
(`ProgramCounter`), peeking and advancing being separate (`InstructionFetcher::peek_instruction()`),
`ExecutionResult`/`ExecutionError` staying within 16 bytes (`PackedAddress`), the outcome of threaded execution coming
back in registers with an explicitly pinned ABI (`OpaqueThreadedExecutionResult`, `handler_abi()` in the emitter) and
handlers being aligned to cache lines. Note that pinning an ABI is not free even where it matches the platform default,
`extern "Rust"` carries parameter attributes an explicit ABI doesn't, which matters when the `TODO` on `handler_abi()`
is revisited.

Settle layout questions with `const { assert!(size_of::<T>() <= N) }` probes rather than by reasoning. Things that
were not obvious: a `(u8, u64)` tuple's padding can't hold the enclosing enum's discriminant while named fields can,
and `repr(packed)` can't be applied to an enum, so the only way to lower its alignment is through its fields, which is
what `PackedAddress` is for (one field at natural alignment is enough to push the enum over the limit again).

## Open ideas

- Direct threading, storing a reference to the handler in the decoded slot instead of the variant: expected to be 10%+
  faster, not generated yet. A hand-written prototype found that an 8-byte slot holding an `i32` handler offset from an
  anchor handler plus 4 operand bytes beats a 16-byte slot with a full pointer, and doesn't grow the decoded stream.
  Operands fit into 4 bytes for every format because B-type offsets are `i16`, each operand has to be read with its own
  byte-aligned load (loading the packed word once and extracting from it costs a spill), and the niches of
  `FetchInstructionResult` and of the dispatch result, free while the jump table absorbs them, become a range check in
  every handler once the table is gone: look there first if a direct-threaded build comes out slower. Per-format
  packing also means the `match` loop can no longer share the decoded stream through `get_rs1_rs2_operands()`.
- Conditional branches currently leave through two dispatch sites, one per outcome, see the comment on the `Branch`
  arm in the emitter. Direct threading or branch targets resolved during decoding remove what keeps them apart, the
  same collapse into one indirect jump that cost Wasmi and Stitch heavily after Rust 1.92. A release build of the
  CoreMark runner can be checked for it without benchmarking, this lists handlers with a conditional move and a single
  dispatch site, which should only be value selects like `min`/`max`, never a branch (on aarch64 look for `csel` and
  `br` instead):
  ```bash
  objdump -d target/release/ab-riscv-coremark-runner \
      | awk '/^[0-9a-f]+ </ {fn = $2} /\tcmov/ {c[fn]++} /\tjmp +\*/ {j[fn]++} \
             END {for (f in j) if (j[f] == 1 && c[f]) print f}' \
      | grep -o 'instruction_[a-z0-9_]*_threaded' | sort -u
  ```
- Decode-time remapping of `rd` = `x0` to a sink slot that is never read would remove run-time `x0` handling
  completely. It needs a change to the shared register types, so it is a design decision rather than an experiment.
- The ~500-variant vector composition has never been benchmarked for dispatch, and it is where the `match` loop is
  expected to suffer most from its size.
- Padding `rs1`/`rs2` fields added to variants that don't have them only serve the `match` loop's preamble, and could
  go away once something like `MaybeUninit::freeze()` (RFC 4001) exists.
- BOLT on top of PGO has not been tried.
