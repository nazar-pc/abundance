# Transactions abstraction

Transactions are a way to run a logic on a blockchain, which in blockchains with smart contracts means to call
contracts. There are different ways to do this, but to make things as flexible as possible, little assumptions are made
about what transaction actually looks like. There are many use cases that should be supported, it is tough to foresee
them all.

Since as mentioned in [Contract overview] "[Everything is a contract]," then the majority of transaction processing must
be done by a contract too. To do this, the contract must implement the following "transaction handler" interface that
looks like this (simplified for readability):

[Contract overview]: Contracts_overview

[Everything is a contract]: Contracts_overview#everything-is-a-contract

```rust
pub struct TransactionHeader {
    pub block_hash: Blake3Hash,
    pub gas_limit: Gas,
    pub contract: Address,
}

pub struct TransactionSlot {
    pub owner: Address,
    pub contract: Address,
}

pub type TxHandlerPayload = [u128];
pub type TxHandlerSlots = [TransactionSlot];
pub type TxHandlerSeal = [u8];

#[contract]
pub trait TxHandler {
    /// Verify a transaction
    #[view]
    fn authorize(
        #[env] env: &Env,
        #[input] header: &TransactionHeader,
        #[input] read_slots: &TxHandlerSlots,
        #[input] write_slots: &TxHandlerSlots,
        #[input] payload: &TxHandlerPayload,
        #[input] seal: &TxHandlerSeal,
    ) -> Result<(), ContractError>;

    /// Execute previously verified transaction
    #[update]
    fn execute(
        #[env] env: &mut Env,
        #[input] header: &TransactionHeader,
        #[input] read_slots: &TxHandlerSlots,
        #[input] write_slots: &TxHandlerSlots,
        #[input] payload: &TxHandlerPayload,
        #[input] seal: &TxHandlerSeal,
    ) -> Result<(), ContractError>;
}
```

High-level transaction processing workflow:

```d2
vars: {
    d2-config: {
        pad: 0
        theme-overrides: {
            N7: transparent
        }
        dark-theme-overrides: {
            N7: transparent
        }
    }
}

direction: right

"TxHandler::authorize()" -> Charge gas -> "TxHandler::execute()" -> Refund gas
```

`TxHandler::authorize()` is a method to be called by execution environment that must, in a limited amount of time,
either authorize further processing of the transaction or reject it. It can read the state of the blockchain, but can't
modify it. If authorized successfully, execution environment will charge `TransactionHeader.gas_limit` gas, call
`TxHandler::execute()` and return unused gas afterward. It is up to the node to decide how much compute to allow in
authorization, but some reasonable for reference hardware default will be used to allow for typical signature
verification needs. Compute involved in transaction authorization will be added to the total gas usage. `seal` is where
the signature will typically be stored, although it is more of a convention a strict requirement.

`TxHandler::execute()` is responsible for transaction execution, meaning making method calls. Method calls by convention
are serialized into `payload` (`u128` is used to ensure its alignment in memory for performance reasons and to enable
zero-copy throughout the system). It is up to the contract how it wants to encode method calls there, though optimized
reference implementation of this is provided. While typically not needed, authorization code may also inspect `payload`
to, for example, only allow certain method calls and not others.

This separation should be enough to build all kinds of contracts that would server as a "wallet" for the user: from
those that do simple signature verification, to complex multisig wallet with a sophisticated role-based permission
system. There is a large space of tradeoffs to explore.

`read_slots` and `write_slots` contain the list of slots (see [Storage model]), which will be read or possibly modified
during transaction execution. They will not need to be inspected by most contracts in detail, though can be used
constrain interation with a limited set of contracts if needed. This information is crucial for to be able to schedule
concurrent execution of non-conflicting transactions, leveraging the fact that modern CPUs have multiple cores. This is
primarily enabled by the storage model that makes the storage used by contracts well suited for concurrent execution by
avoiding data structures like global hashmaps that are likely to be updated by multiple transactions in a block.

[Storage model]: Contracts_overview#storage-model

# Transaction processing

A transaction submitted to the network will include not only inputs to the method calls, the storage items (code, state,
other slots) required for the transaction to be processed alongside corresponding storage proofs. This allows for
consensus nodes to not store state of contracts beyond a small root, yet being able to process incoming transactions,
leading to much lower disk requirements. This is especially true in the presence of dormant contracts without any
activity for a long period of time and generally removes the need to charge "rent" for the state. It is, of course,
possible for node to have a cache to reduce or remove the need to download frequently used storage items.

Each method call of the contract has [metadata] associated with it about what slots will be read or modified alongside
any inputs or outputs it expects and their type information. With this information and `read_slots`/`write_slots`
included in the transaction, execution engine can run non-conflicting transactions in parallel.

[metadata]: Contracts_overview#metadata

For example, balance transfer between two accounts doesn't change the total issuance of the token. So there is no need
to change the global state of the token contract and no reason why such transfers affecting a disjoint set of accounts
can't be run in parallel.

Not only that, storage items used in each method call follow a Rust-like ownership model where contract can't
recursively call its own method that mutates already accessed slots because it'll violate safety invariants. Recursive
calls of stateless or read-only methods are fine though.

The right mental model is that storage access can be used with shared `&` or exclusive `&mut` references. It is possible
to have multiple shared references to the same slot at the same time. For exclusive access in a recursive call to the
slot already being accessed, caller must share it as an input instead, explicitly giving borrowing the data. As a
result, multiple calls (in the same transaction or even different transaction) can read the same slot concurrently, but
only one of them is allowed to mutate a particular storage item at a time. And any violation aborts the corresponding
method, which caller can observe and either handle or propagate further up the stack.

This makes traditional reentrancy attacks impossible in such execution environment.

Conceptually in pseudocode with `RwLock` it looks something like this:

```rust
fn entrypoint(data: &RwLock<Data>) -> Result<(), Error> {
    // This is the first write access, it succeeds
    let data_write_guard = data.try_write()?;

    // This will fail because we still have write access to the data
    if call_into_other_contract(data).is_err() {
        // This is okay, the data was given as an explicit argument
        modify_data(data_write_guard);
    }

    Ok(())
}

fn call_into_other_contract(data: &RwLock<Data>) -> Result<(), Error> {
    // Only succeeds if there isn't already write access elsewhere
    data.try_read()?;

    Ok(())
}

fn modify_data(data: &mut Data) {}
```

Here is a visual example:

```d2
vars: {
    d2-config: {
        pad: 0
        theme-overrides: {
            N7: transparent
        }
        dark-theme-overrides: {
            N7: transparent
        }
    }
}

direction: right

Stateless: No state (Contract 1) {
    compute: fn compute(...)
}

Mutates: Mutates own state (Contract 2) {
    update: fn update(&mut self, ...)
}

Reads: Reads state (Contract 3) {
    read: fn read(&self, ...)
}

Stateless.compute -> Stateless.compute: ✅
Stateless.compute -> Mutates.update: ✅
Mutates.update -> Mutates.update: ❌
Mutates.update -> Reads.read: ✅
Reads.read -> Reads.read: ✅

```

Such a loop will be caught and the transaction will be aborted:

```d2
vars: {
    d2-config: {
        pad: 0
        theme-overrides: {
            N7: transparent
        }
        dark-theme-overrides: {
            N7: transparent
        }
    }
}

direction: right

Mutates: Mutates own state (Contract 1) {
    update: fn update(&mut self, ...)
}

Reads: Reads state (Contract 2) {
    read: fn read(&self, ...)
}

Mutates.update -> Reads.read: ✅ {
    source-arrowhead: Start
}
Reads.read -> Mutates.update: ❌

```
