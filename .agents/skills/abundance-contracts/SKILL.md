---
name: abundance-contracts
description: Design intent and trust model of contract execution in this repository. Use when working on or reviewing anything under `crates/contracts`, `ab-io-type`/`ab-io-type-derive`, the executors (`ab-executor-*`), contract metadata and method fingerprints, `ExternalArgs`/`PreparedMethod`/`Env`, the simple wallet contracts, or transaction payload handling, and especially before calling something in these crates "unsound" or a bug. Not for the RISC-V interpreter and primitives internals (`ab-riscv-*`), which the `abundance-riscv` skill covers.
---

# Contracts and execution in Abundance

Most wrong proposals in this area came from judging the code against a threat model it doesn't have, or from adding
checks to a layer that isn't supposed to do them. Read the trust documentation that already exists before calling
anything unsound: `NativeExecutor`, `PreparedMethod` and `PreparedMethod::new_untyped()`, `ExternalArgs`,
`ExecutorContext`, `SimpleWalletBase::execute()` and `TransactionPayloadCaller`. The book under
`website/book/src/Execution_environment/` explains the contract and transaction model, including what callee-generated
code checks.

## Who checks what

A `PreparedMethod` carries the method fingerprint, which stands for the API the callee expects. A caller that knows the
fingerprint knows the ABI, so it is responsible for building `ExternalArgs` correctly. Where it gets that knowledge from
is irrelevant: usually from the generated code of the callee crate added as a dependency, while the wallet is the
special case that has no metadata at hand but does know sizes and capacities from the payload.

Executors are the ones that guarantee that sizes, capacities and alignment of arguments match the method's metadata:

- The RISC-V VM (the intended production executor) reads the caller's memory and copies it to the callee according to
  metadata, checking it on the way. The host knows the metadata of every call anyway, so it must check regardless. The
  interpreter is not wired into a full execution VM yet, only the benchmarks run contract code in it, so docs that say
  "the RISC-V VM checks" describe the design.
- `NativeExecutor` passes pointers and trusts contracts and the data they forward, transaction payloads included.
  Mismatches are undefined behavior there, by design. It exists for peak native performance and is turning into a
  testing/debugging/safety-checking implementation: running it under Miri catches unsound code on both sides of a call.
  Don't "fix" this by adding release-mode checks to it.

The callee parses the *contents* of `#[input]` and `#[output]` arguments and returns `ContractError::BadInput` for
invalid values. Size, capacity and alignment checks in the callee are debug assertions only, since in release they are
pure overhead that the VM already covers. Likewise, the return value has no size in `ExternalArgs` on purpose (its type
fixes it), and argument counts are not duplicated into `PreparedMethod`: that was considered and rejected as extra
defensive, it makes no difference to the VM or the callee.

The threat model for wallets: a payload that passed `authorize()` was signed by the owner and is trusted.
"Intentional self-harm is essentially allowed for wallet owners", there are other ways for them to achieve it anyway. A
malicious payload the user was tricked into signing is not something the wallet's execution has to defend against; the
defense is clear signing (below).

## Transactions: authorization, execution, clear signing

Transactions go through `TxHandler::authorize()` and then `TxHandler::execute()`. Authorization can be parallelized
widely, has a limited compute allowance and its cost is added to gas; execution is less parallel. Verification that only
depends on the payload therefore belongs in authorization where possible, and execution shouldn't repeat it. The wallet
doesn't take full advantage of this split yet, it's a known follow-up rather than an oversight to fix ad hoc. This is
also why the wallet has both `TransactionPayloadDecoder` (checks and describes a payload without calling anything) and
`TransactionPayloadCaller`, sharing one decoding core.

Metadata exists so that fully clear signing is possible: what the user signs and what they think they sign (for example
on a hardware wallet screen) match exactly. Clear signing will have verified metadata and can render the contents.
Execution must not pay for that: don't make it decode metadata, don't embed metadata in transactions, and don't make the
decoder return information the execution path would compute and then discard if dead-code elimination can't remove it.

## `ab-io-type`

`ab-io-type` is a low-level serialization crate and must stay independent of contract concepts. Contract-specific
pieces built on it (`OutputArg`, `Lent`, `Env::lend_output()`) live in `ab-contracts-common`; don't move them down or
add contract-aware traits to `ab-io-type`.

- `TrivialType` values are parsed, which is a length check only for types that accept every bit pattern. `AnyBitPattern`
  is the bound to use for safe access without parsing. It already implies `TrivialType`, so don't write
  `TrivialType + AnyBitPattern`, and `AnyBitPatternMarker` only exists because of a compiler limitation (see its
  `TODO`). For an `#[output]` whose type doesn't accept every bit pattern, `Lent` is the mechanism that parses after
  each call.
- Alignment: `Alignment::MAX` (16 bytes) is the largest alignment any `TrivialType` has, and it is defined on its own,
  not as "the alignment of `u128`". Using `u128` to get 16-byte alignment is a historical artifact; new code uses
  `Alignment` and, for raw buffers, a dedicated `#[repr(C, align(16))]` chunk type like the wallet's `OutputChunk`.
  Don't scatter explanations of `u128` being 8-byte aligned on 32-bit targets through docs. `TrivialType`s are
  composites of existing `TrivialType`s without `#[repr(align(N))]`/`#[repr(packed)]` (see the derive's requirements),
  reduce alignment with `Unaligned<T>` instead.
- Generic code using the `[T; N]` impl hits a bound that rustc's diagnostics say needs `generic_const_exprs`. That
  suggestion is outdated: enable the `gca_*` features (`gca_const_items` and friends) the way other crates, e.g.
  `ab-erasure-coding`, do.

## Misaligned access in contracts

The contract guest target (`ab-contracts-tooling`'s `riscv64-unknown-none-abundance.json`) enables
`+unaligned-scalar-mem`, so misaligned scalar reads are supported in guests. Contract file bytes have no alignment
guarantee and are read through `Unaligned<T>`, which is the public API for unaligned data.
`ContractInstruction::ALIGNMENT` is 2, not the `4` written in its own impl: the `#[instruction]` macro takes the
smallest alignment among the inherited instruction sets, and those include compressed instructions.

## Tests

Contract tests run on `NativeExecutor`, which is also how soundness problems get caught under Miri. Tests belong in the
crate that owns the functionality: `ab-system-contract-simple-wallet-base` is the canonical wallet implementation with
the payload, seal and integration tests, while `ab-example-contract-wallet` is just a small adapter over it, so wallet
behavior is not tested there.

`#[contract]` ui tests live in `ab-contracts-macros/tests/ui/<concept>/`: one file per rule, with a `//!` header
stating the rule and why it exists, and one `mod` per case, since the macro stops at the first error. One case per
rule a user can plausibly hit, pinning the macro's own message rather than rustc's where possible.
`tests/metadata_contents.rs` is the positive counterpart, showing what each accepted form turns into in metadata.

CI runs these tests with and without the `guest` feature, and the same `.stderr` must match both, so run both locally.
With `guest`, every contract and trait exports its metadata through a `#[no_mangle]` static named after it, whose type
reads `METADATA`. That changes when rustc evaluates metadata, so a check moved into `METADATA` can reorder diagnostics
between the two runs, and two contracts with the same name can't be in one test binary (tests that need that are
compiled out with `#[cfg(not(feature = "guest"))]`).
