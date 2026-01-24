---
title: Transactions
date: 2025-03-02
draft: false
description: Initial work on transactions and other updates
tags: [ status-update ]
authors: [ nazar-pc ]
---

The most important progress from last week is initial work on transactions. I've spent quite some time thinking about
the design and even implemented an initial wallet contract alongside with related infrastructure.

<!--more-->

As mentioned at the end of last week, adding a notion of transactions and explicit slots were the next steps and the
first part of that is now implemented in [PR 79], but first a bit of context.

[PR 79]: https://github.com/nazar-pc/abundance/pull/79

As mentioned [in the book], "Everything is a contract" is the design philosophy for many things, and wallets are not an
exception here. This basically means that there is no "system-wide" notion way of a signature scheme to use for
transactions or even a way to serialize method calls into transaction payload. The wallet is just a contract that must
conform to some fairly generic interface. This interface should be flexible for all kinds of wallets: from simple ones
with a public key and nonce that checks a signature and supports simple transactions to complex multisig wallets, 2FA
support, support for whitelisting/blacklisting transactions depending on signer and a lot of other things I probably
can't think of right now. At the same time, contracts should compile to compact RISC-V binary and not require heap
allocation in most cases, ideally taking advantage of zero-copy mechanisms whenever possible.

[in the book]: /book/Execution_environment/Contracts_overview.html

As a result, I came with a trait that looks something like this (a bit simplified for this article):

```rust
pub struct TransactionHeader {
    pub genesis_hash: Blake3Hash,
    pub block_hash: Blake3Hash,
    pub gas_limit: Gas,
    pub contract: Address,
}

pub type TxHandlerPayload = [u128];
pub type TxHandlerSeal = [u8];

pub trait TxHandler {
    /// Verify a transaction
    #[view]
    fn authorize(
        env: &Env,
        header: &TransactionHeader,
        payload: &TxHandlerPayload,
        seal: &TxHandlerSeal,
    ) -> Result<(), ContractError>;

    /// Execute previously verified transaction
    #[update]
    fn execute(
        env: &mut Env<'_>,
        header: &TransactionHeader,
        payload: &TxHandlerPayload,
        seal: &TxHandlerSeal,
    ) -> Result<(), ContractError>;
}
```

`TxHandler::authorize()` takes transaction header, payload and seal and must make a decision whether to authorize
transaction or not. Authorization implies that the cost of gas limit will be charged before calling
`TxHandler::execute()` and the remainder will be returned after it.

Essentially, what we have is an interface that a node (transaction pool and execution environment) will be aware of to
statelessly verify the transaction for validity and stateful way to actually execute it. The contents of a transaction
payload is opaque to the execution environment (but has to be aligned to 16 bytes) just like seal is.

The payload canonically contains serialized method calls. Each wallet is allowed to implement it, whichever way it
wants, but there are some utilities in `ab-system-contract-simple-wallet-base` crate/contract that provide a reference
implementation of what it might look like. Specifically, that crate supports sequences of transactions and the ability
to reference outputs of previous transactions in transactions that follow, which is important for contract deployment,
for example.

The payload is aligned to the maximum alignment supported by `TrivialType` that is used for I/O between host and guest
with inputs, this way reference serialization/deserialization of method calls ensures all data structures are correctly
aligned in memory. Aligned data structures mean they don't need to be copied, the same bytes that were received from the
networking stack could be passed around as pointers and sliced into smaller data structures without allocating any more
memory dynamically.

The seal canonically contains something that authorizes the transaction, like a cryptographic signature and nonce to
prevent transaction replaying. `ab-system-contract-simple-wallet-base` literally has those, but one can have more than
one signature, some kind of one-time token instead of nonce and all kinds of other things imaginable.

Authorization here is a custom code and its execution is not guaranteed to be paid for, so how is it handled?

Well, with transaction signatures being a notion of the blockchain node, the same issue exists. So the answer here is
"it depends," specifically node should be able to configure its own limit, but once the transaction is in the block,
inability to pay for it will make block invalid. It is expected that initially some low-ish limit will be set that is
enough to verify afew signatures, but it may be increased over time, including by node operator. This provides the
ultimate flexibility for contract developers while reducing the complexity of the node implementation.

`ab-system-contract-simple-wallet-base` is also deployed as a system contract, containing the foundational logic, while
I also added `ab-contract-example-wallet` that demonstrates how to take advantage of it to have a compact and efficient
wallet contract.

## Hardware wallets

I'd like to dedicate a whole separate section for hardware wallets, especially in the context of recent [ByBit hack].

[ByBit hack]: https://x.com/Bybit_Official/status/1892965292931702929

One of the first things in the design of contracts was the question of how to efficiently represent data structures in
serialized form. On the one hand, it is desirable to be able to pass data structures in zero-copy manner as much as
possible; on the other hand, there should be a way to make sense of them. This is why I initially reached to `zerocopy`
crate, which had tooling for this, but [didn't have metadata generation] utilities like [SCALE has]. I also looked at
`musli-zerocopy`, which was another promising candidate, but required a git awkward wrappers and still didn't solve the
metadata generation/parsing issue.

[didn't have metadata generation]: https://github.com/google/zerocopy/issues/2184

[SCALE has]: https://github.com/paritytech?q=scale&language=rust

In the end, [`TrivialType`] trait was born (implemented for types that can be treated as a bunch of bytes like `u8`,
`[u32; 4]`, etc.) and [`IoType`] that is implemented for `TrivialType` and a few custom data structures. `TrivialType`
can be derived, and derived trait will contain `const METADATA`. This metadata can describe all kinds of data structure
shapes that can be passed between host and guest environment as "bytes," meaning no serialization code is necessary,
just a pointer to existing memory.

[`TrivialType`]: /rust-docs/ab_contracts_io_type/trivial_type/trait.TrivialType.html

[`IoType`]: /rust-docs/ab_contracts_io_type/trait.IoType.html

[`#[contract]`][contract] macro also implements metadata, but this time for the all methods, which includes data
structures involved too. As the result, all of this information is put into `ELF` section to be uploaded to the
blockchain together with the code and can be read by various tools.

[contract]: /rust-docs/ab_contracts_macros/attr.contract.html

There is a bunch of places that read the metadata to make sense of the data structures various methods expect. Execution
environment uses it to decode data structure received from one contract and to generate another data structure when
calling another. Similarly `ab-system-contract-simple-wallet-base` uses it to serialize/deserialize method calls to/from
payload bytes.

Going back to the hardware wallets and blind signing that lead to the hack, it would be possible to actually **both
display and verify** in somewhat human-readable format the contents of every transaction right on the hardware wallet
itself. Especially with wallets like [Ledger Stax], there is plenty of space to do so.

[Ledger Stax]: https://shop.ledger.com/products/ledger-stax

This is how it can be done:

* each transaction header contains both genesis hash and block hash for which transaction is signed
* genesis hash is enough to know which blockchain transaction is signed for
* from block hash it is possible to generate proofs about metadata of all the contracts involved in a particular
  transaction
* since hardware wallet can confirm what contract it is signing transaction for, it can also decode, display and verify
  its contents, for example, with utilities provided by `ab-system-contract-simple-wallet-base`

As a result, there is no blind signing, no need to trust the UI or machine that the wallet is connected to.

For now the wallet would have to know which kind of wallet contract is used or else it'll not know how to deserialize
opaque `payload` (which is a price to pay for utmost flexibility of transaction format).

## Upcoming plans

There was a lot of preparation work and lower-level API changes done that led to the transaction interface, but I will
not bother readers with it this time because the blog post is fairly large as is.

As mentioned in the previous update, the next big step will be to integrate this into execution environment. And there
are many conveniences to add and paper-cuts to eliminate here and there, after which I'll be looking to do more
developer interviews.

Speaking about interviews, I had a technical interview with one of the candidates last week and hoping to have good news
to share next time.

This was one long blog post, if you made it till the end, thank you and see you next in about one more week!
