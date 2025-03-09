---
title: There is two of us now
date: 2025-03-09
draft: false
description: Extremely valuable addition to the team and performance tuning updates
tags: [ announcement, status-update ]
authors: [ nazar-pc ]
---

The big change from the last update is that [Alfonso de la Rocha](https://www.linkedin.com/in/adlrocha/) has joined me
as a part-time researcher to help with sharding designing. Code-wise, there were also a bunch of performance benchmarks
and optimizations.

<!--more-->

Alfonso has worked extensively with blockchain-related tech and research, most recently at Protocol Labs. At PL he
worked on Interplanetary Consensus (IPC) and a bunch of other things, that are all one way or another relevant to
high-performance blockchains. Currently, he is learning how Subspace consensus works and prepares a framework for
reasoning about sharding design options. Starting next week, there should be a section with updates from him too, so
stay tuned.

I'm very excited and I know he is too!

## Transactions

Last time I mentioned that a notion of transactions was added, but not yet integrated. I'm happy to report that it
finally happened in [PR 39]!

[PR 39]: https://github.com/nazar-pc/abundance/pull/89

Instead of methods to manipulate environment directly, there are now dedicated methods for verification and execution of
transactions using `TxHandler` interface that a wallet contract is supposed to implement. Crafting transactions for
testing purposes might be tedious, so there is a transaction emulation API now for that purpose.
`NativeExecutor::env_ro()` method was retained to be used for calling stateless methods, like when processing RPC
requests, etc.

Executor creation was converted into a builder and storage container was extracted out of the executor instance, in the
future it will have persistence APIs such that state can be persisted on disk and read back later. Just like you'd
expect a normal blockchain node to do it, but we're not quite there yet.

The same PR also introduced some test utilities in a separate crate, for example, a `DummyWallet`.

[PR 94], [PR 100] and [PR 101] updated transaction structure in preparation for work on the transaction pool, but no
work on that has started yet, just some preliminary research.

[PR 94]: https://github.com/nazar-pc/abundance/pull/94

[PR 100]: https://github.com/nazar-pc/abundance/pull/100

[PR 101]: https://github.com/nazar-pc/abundance/pull/101

## Performance

In [PR 92] I introduced benchmarking for example wallet implementation, discovering that signature verification
massively dominates transaction processing time at ~14 µs 🤔. Not yet sure what to do with that, feels expensive, and it
already takes advantage of AVX512 instructions on my Zen 4 CPU. Will have to think about that some more.

[PR 92]: https://github.com/nazar-pc/abundance/pull/92

Equipped with benchmarks, I spent a lot of time thinking and experimenting and came to the conclusion that while making
multi-calls to leverage CPU concurrency is a sound idea in general, it needs to go. The reason is that instantiation
cost is non-negligible and with a VM it will become even larger. At the same time, the amount of compute that can be
done in a transaction in general isn't so large that splitting it into multiple threads would be critical.

Not having multi-calls simplified the design substantially, which after many trials and errors and long hours finally
resulted in [PR 103] and follow-ups [PR 104] + [PR 106].

[PR 103]: https://github.com/nazar-pc/abundance/pull/103

[PR 104]: https://github.com/nazar-pc/abundance/pull/104

[PR 106]: https://github.com/nazar-pc/abundance/pull/106

Also [learned something] a bit surprising about `Self` and subtyping in Rust.

[learned something]: https://users.rust-lang.org/t/return-mutable-reference-to-a-field-from-mut-self/126705?u=nazar-pc

The latest numbers for execution environment for direct calls and transaction emulation overhead look like this:

```
flipper/direct          time:   [76.389 ns 76.696 ns 77.047 ns]
                        thrpt:  [12.979 Melem/s 13.038 Melem/s 13.091 Melem/s]
flipper/transaction     time:   [91.724 ns 91.936 ns 92.188 ns]
                        thrpt:  [10.847 Melem/s 10.877 Melem/s 10.902 Melem/s]
```

13 Million flips per second is A LOT more than 5 that I mentioned in previous updates, this is how much faster it
became, very pleased with the results so far.

As for going through the while transaction processing pipeline for verification and execution of a well-formed
transaction, the numbers look like this:

```
example-wallet/verify-only
                        time:   [18.268 µs 18.304 µs 18.345 µs]
                        thrpt:  [54.512 Kelem/s 54.634 Kelem/s 54.742 Kelem/s]
example-wallet/execute-only
                        time:   [3.1408 µs 3.1470 µs 3.1545 µs]
                        thrpt:  [317.01 Kelem/s 317.76 Kelem/s 318.39 Kelem/s]
```

Yeah, 54k transactions can be verified per second, while over 300k simple transactions can be executed per second. And
this is all on a SINGLE CPU core. Even accounting for VM overhead, the ceiling is very high. The challenge will be to
feed all these transactions at this rate (there is networking, disk access, lots of things that may slow this down).

## Other improvements

To increase the robustness of the features, [PR 90] extended GitHub Actions workflows with more cases enabling various
features and making sure they actually work properly.

[PR 90]: https://github.com/nazar-pc/abundance/pull/90

Since the number of crates increased, [PR 98] rearranged crates into more subdirectories to help with navigation and
ease discoverability of example and system contracts. Shamil will be pleased, I'm sure 😉. Some other of his feedback was
regarding complexity and confusion regarding `#[output]` vs `#[result]`. After thinking about it and trying a few
things, [PR 83] and then [PR 84] removed `#[result]`, unifying the code in the process, which I hope will be not too
convoluted and a bit easier to maintain. Also from him was a suggestion to support user-defined errors, which [PR 82]
implemented as well (with some further API improvements possible too).

[PR 98]: https://github.com/nazar-pc/abundance/pull/98

[PR 83]: https://github.com/nazar-pc/abundance/pull/83

[PR 84]: https://github.com/nazar-pc/abundance/pull/84

[PR 82]: https://github.com/nazar-pc/abundance/pull/82

And a small but nice addition, [PR 102] started copying original documentation (and linter attributes) from contract
methods to generated extension trait methods.

[PR 102]: https://github.com/nazar-pc/abundance/pull/102

## Upcoming plans

This has been a productive week, and I'm sure the next one will be too. I'm looking forward to transaction pool
implementation, but not sure if I have sufficient infrastructure for that yet or this is the right time.

I'll be writing more documentation in the book about transactions and how they are processed now that it has solidified
a bit.

And I'd really like to have one or two developer interviews this week if possible to collect more feedback.

As usual, thank you for reading and until next time!
