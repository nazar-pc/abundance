---
title: Way faster transactions and no-panic
date: 2025-03-17
draft: false
description: Performance optimizations for transactions and other improvements
tags: [ status-update ]
authors: [ nazar-pc ]
---

The plan was to get to transaction pool implementation, but it didn't quite happen. I did a lot of investigation around
performance though. For example, transaction processing was several orders of magnitude slower than direct method calls
without a transaction, which concerned me, but after optimizations of last week the difference is ~10x. And it makes
sense given how much more work the wallet has to do on top of the method call itself.

<!--more-->

So last week we ended up with ~300 k transaction per second processing rate, while direct method calls were at ~13 M/s
and transaction emulation (bypassing the wallet implementation) was at ~10.8 M/s. But why?

This turned out to be a combination of things, but the most impactful change was, funnily enough, probably a single
stack-allocated variable that had a size 128x larger than expected. It was documented to be using byte units, but was
actually expressing number of `u128`s. Decreasing it 128 times helped a lot. There was another data structure that was
also 32 kiB in size, while needing less than one, all that and some more was corrected in [PR 120].

[PR 120]: https://github.com/nazar-pc/abundance/pull/120

That PR also introduced a constant `MAX_TOTAL_METHOD_ARGS` to define the maximum number of arguments the method can
have. It was not constrained previously, which didn't allow making assumptions about the max size of data structures and
required heap allocations in executor. It is limited to 8, which is the number of bits in a byte and led to much
anticipated rewrite of transaction payload encoding.

The encoding worked but was a bit less compact than I'd like it to be and didn't support using outputs of previous
method calls in the slots of the next one. Now that we know that there couldn't be more than 8 arguments in a method, it
is possible to use very compact bit flags to indicate what kind of value slot or input will use.

[PR 121] implemented the changes and fixed a few bugs. Specifically, it is now possible to create a transaction that in
pseudocode looks like this:

[PR 121]: https://github.com/nazar-pc/abundance/pull/121

```
wallet_address = Code::deploy(wallet_code)
Wallet::initialize(wallet_address, public_key)
Token::send_to(wallet_address, amount)
```

Note that in token transfer `wallet_address` might be a slot, and that is exactly what wasn't supported previously.
While only a simple sequence of method calls is supported by this reference transaction payload encoding, it already
allows creating interesting workflows. For example, in Ethereum you often use approval in DeFi applications to allow
spending your tokens by a contract. Here it would be possible to allow spending specific number of tokens just for the
duration of the transaction itself:

```
Token::allow_transfer_once_by(Context::Reset, contract_addres, amount)
Defi::do_something(Context::Reset)
```

Context wasn't mentioned in examples before for simplicity, but it essentially is a notion similar to a "user" in
operating systems, defining what things can be accessed. It is possible to inherit the context from a caller, override
it with itself or reset to `Address::NULL`. Resetting means you don't need to worry much about whatever contract you're
calling. They can't do anything on behalf of the wallet due to context being `Address::NULL` already. However, by doing
temporary approval, `Defi` contract gains an ability to do something with `Token`, but only within the limits of
explicit approval. This is a much nicer security model that is easier to reason about IMO. And since all inputs and
outputs are explicit, there is no danger of contracts messing up with wallet's state by accident.

Something that `Token::allow_transfer_once_by()` above would use is `#[tmp]` argument for ephemeral storage (for
duration of the transaction only), but implementation-wise it is abusing `Address::NULL` slots, which unfortunately were
not cleaned up properly before. [PR 115] finally extracted `Slots` data structure along with underlying aligned buffers
into a separate crate, which will be needed for transaction pool implementation and [PR 119] implemented cleanup for
`#[tmp]` "slots".

[PR 115]: https://github.com/nazar-pc/abundance/pull/115

[PR 119]: https://github.com/nazar-pc/abundance/pull/119

## Performance

With transactions being much faster, I was exploring other kinds of overhead. This time the focus was on compactness and
avoiding heap allocations, with [PR 114], [PR 124] and [PR 125] performance looks roughly like this:

[PR 114]: https://github.com/nazar-pc/abundance/pull/114

[PR 124]: https://github.com/nazar-pc/abundance/pull/124

[PR 125]: https://github.com/nazar-pc/abundance/pull/125

```
flipper/direct          time:   [48.326 ns 48.469 ns 48.638 ns]
                        thrpt:  [20.560 Melem/s 20.632 Melem/s 20.693 Melem/s]
flipper/transaction     time:   [57.682 ns 57.933 ns 58.219 ns]
                        thrpt:  [17.177 Melem/s 17.261 Melem/s 17.336 Melem/s]
example-wallet/execute-only
                        time:   [535.60 ns 537.22 ns 539.44 ns]
                        thrpt:  [1.8538 Melem/s 1.8614 Melem/s 1.8671 Melem/s]
```

A much smaller gap between direct method call and transaction emulation with a massive difference from last week. But
more impressive is increase from ~300 k/s to 1.8 M/s for transactions that can be processed through the whole things,
including the wallet. It is still bottlenecked by the signature verification cost, of course.

These can and will improve further. For example, metadata decoding is currently happening on every method call, but it
can be cached to remove a significant amount of compute overhead from all benchmarks above.

## No panic

There is an interesting crate in Rust ecosystem called [`no-panic`]. What it does is prevent code from compiling unless
the compiler can be convinced that annotated method can't possibly panic. This is a very nice property for assuring
reliability and predictable code behavior. It is a rock-solid compiler guarantee that the API is what it appears to be.
While an error handling story in Rust is already great, the fact that certain things can potentially panic is still
really annoying.

[`no-panic`]: https://github.com/dtolnay/no-panic

The way it works is basically inserting a wrapper around user code that instantiates a struct that calls non-existing
function in `Drop` implementation (that would cause linking error) before user code and removing instance after to
prevent from `Drop` from actually running. The only way compiler would not eliminate that instance and its `Drop` it as
dead code is if the user code can panic, in which case the `Drop::drop()` would need to be called during unwinding.
Brilliant!

It is tricky to use, but with some effort can be applied to non-const (for now) methods. [PR 109] implemented support
for panic-free payload decoding in `ab-system-contract-wallet-base` and [PR 118] extended this to a large portion of
`ab-contracts-common` API. Really looking forward to trait support in `const fn` in Rust to be able to apply this to
many `const fn` functions and methods in the codebase. I'll also extend annotations to more crates over time.

[PR 109]: https://github.com/nazar-pc/abundance/pull/109

[PR 118]: https://github.com/nazar-pc/abundance/pull/118

---

## Documentation

I need to work some more on documentation, and last week I added [Transaction overview] page to the book with some more
details now that things have settled a bit.

[Transaction overview]: /book/Execution_environment/Transactions_overview.html

## ELF

I've spent some time researching on what format contracts should be packaged in. I thought about standard ELF rather
than something custom for a long time, but didn't know the specifics. It'd be great to use `cargo build`'s output, even
if with custom configuration, without any manual post-processing. I'm still not 100% confident that it'll be possible,
but so far it doesn't seem impossible either.

For a bit of context, what I'm ideally looking for is a 64-bit RISC-V ELF file that can be both uploaded to the
blockchain as a contract and used as a "shared library" of sorts directly. By directly I mean with `dlopen()` or loading
directly into `gdb()`, which will be an excellent debugging experience for developers and will eliminate the need to
have custom compilers and debuggers, using industry standard tooling.

If it works out like I expect, it'd be possible to describe a method call in a format close to the pseudocode I have
shown toward the beginning of this post and call it on a contract in `gdb`/`lldb`. Even IDE integration should work with
precompiled contracts that way without the need to write special IDE-specific plugins, etc. It'll also hopefully open
the door for hardware acceleration on RISC-V hardware by running contracts in a VM, but I'm probably speculating too
much at this point.

I might start with something simpler and try to upload native x86-64 binaries in the format similar to the eventual
RISC-V binaries, so I can get a feel of it and open them with `dlopen()` for now.

I looked into VMs too, with [PolkaVM] looking interesting, but requiring PolkaVM-specific sections in ELF, which I'd
really prefer to avoid. [Embive] looked interesting too, but has different design goals, and I'm not so sure about its
performance. The impression right now is that either some PolkaVM changes would be needed to integrate it or a separate
interpreter/VM will need to be designed instead.

[PolkaVM]: https://github.com/paritytech/polkavm

[Embive]: https://github.com/embive/embive

## Upcoming plans

I'd like to get to transaction pool some time this week and to write more docs. There are also some developer interviews
scheduled for later this week, which I hope will provide useful insights.

I hope you don't regret spending time to read this too much, see you in about a week.
