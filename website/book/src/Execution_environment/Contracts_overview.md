# Everything is a contract

In contrast to many blockchains, the address is just a monotonically increasing number, decoupled from a public key (in
case of end user wallet) or code (in case of what is typically understood as "smart contract" in other blockchains).

The address is allocated on account creation and doesn't change regardless of how the contract evolves in the future.
This means that externally, all contracts essentially look the same regardless of what they represent. This enables a
wallet contract to change its logic from verifying a single signature to multisig to 2FA to use a completely different
cryptography in the future, all while retaining its address/identity.

This not only includes contracts created/deployed by users/developers, but also some fundamental blockchain features.
For example, in most blockchains code of the contract is stored in a special location by the node retrieves before
processing a transaction. Here code is managed by a system `code` contract instead of the node as such, and deployment
of a new contract is a call to system `code` contract instead of special host function provided by the node and `code`
contract will store the code in the corresponding slot of the newly created contract (see [Storage model] below for more
details).

[Storage model]: #storage-model

A few examples of contracts:

* a wallet (can be something simple that only checks signature or a complex smart wallet with multisig/2FA)
* utility functions that offer some shared logic like exotic signature verification
* various kinds of tokens, including native token of the blockchain itself
* even fundamental pieces of logic that allocate addresses and deploy other contracts are contracts themselves

It'll be clear later how far this concept can be stretched, but so far the potential is quite high to make as many
things as possible "just a contract."

This helps to reduce the number of special cases for built-in functions vs. something that blockchain user can deploy.

# Storage model

All storage owned by a contract is organized into a container that has slots inside. It forms a tree with the root
being the root of contract's storage, which can be used to generation inclusion/exclusion proofs when processing
transactions (see [Transaction processing]). Having a per-contract tree with storage proofs allows consensus nodes to
not be required to store the state of all contracts, just their storage roots. This is unlike many other blockchains
where contract may have access to a form of key-value database.

[Transaction processing]: #transaction-processing

Each slot is managed by exactly one of the existing contracts and can only be read or modified by that contract.
Contract's code and state are also slots managed by contracts (system contracts), even though developer-facing API might
abstract it in a more friendly way. It is possible for a contract to manage one of its slots too, like when a token
contract owns some number of its own tokens.

In contract to most other blockchains by "state" we refer to the inherent state of the contract itself, rather than
things that might belong to end-users. The right mental model is to think of it as a global state of a contract.

Let's take a generic fungible token as an example. System `state` contract will manage its state, stored in
corresponding slot owned by the token contract. State will contain things like total supply and potentially useful
metadata like number of decimal places and ticker, but not balances of individual users.

In contrast to most blockchains, the state of the contract is typically bounded in size and defined by contract
developer upfront. Bounded size allows execution environment to allocate the necessary amount of memory and to limit the
amount of data that potentially needs to be sent with the transaction over the network (see [Transaction processing]).

This implies there can't be more traditional unbounded hashmap there. Instead, balances are stored in slots of contracts
that own the balance (like smart wallet owned by end user), but managed by the token contract. This is similar to how
contract's state and code are managed by corresponding system contracts.

Visually, it looks something like this:

```d2
vars: {
    d2-config: {
        pad: 0
        theme-id: 1
        dark-theme-id: 200
        theme-overrides: {
            N7: transparent
        }
        dark-theme-overrides: {
            N7: transparent
        }
    }
}

direction: right

Wallet: Wallet {
    State: State
    Balance: Balance
    Code: Code
}

CodeContract: Code contract {
    Code
}

StateContract: State contract {
    Code
}

Token: Token contract {
    State
    Code
}

CodeContract -> Wallet.Code
CodeContract -> CodeContract.Code
CodeContract -> StateContract.Code
CodeContract -> Token.Code
StateContract -> Wallet.State
StateContract -> Token.State
Token -> Wallet.Balance
```

Contracts do not have access to underlying storage implementation in the form of key-value database, instead they modify
slots as the only way of persisting data between transactions.

# Transaction processing

A transaction submitted to the network includes not only inputs to the contract call, but also storage proofs of the
storage items (code, state, other slots) required for the transaction to be processed alongside corresponding storage
proofs. This allows nodes to not store state of contracts beyond a small root, yet being able to process incoming
transactions (it is of course possible to have a cache to remove the need to download frequently used storage items).

Each method call of the contract includes metadata about what slots it will read or modify alongside any inputs or
outputs it expects and their type information. With this information, contract execution engine can run non-conflicting
transactions in parallel.

Not only that, it can follow the chain of calls ensuring a Rust-like ownership model where contract can't recursively
call its own method that mutates slots because it'll violate safety invariants. Recursive calls of stateless or
read-only methods are fine though.

The right mental model is that storage is contained within `RwLock` and each slot read/write results in
`RwLock::try_read()`/`RwLock::try_write()`. As a result, multiple methods can read the same data concurrently, but only
if nothing tries to write there at the same time. This rule applies through recursive methods calls into other
contracts, and any violation aborts the corresponding method, which caller can observe and either handle or propagate
further up the stack.

This makes traditional reentrancy attacks impossible in such execution environment.

Conceptually in pseudocode it looks something like this:

```rust
fn entrypoint(data: &RwLock<Data>) -> Result<(), Error> {
    // This is the first lock acquisition, it succeeds
    let data_write_guard = data.try_write()?;

    // This will fail because we still have write access to the data
    if call_into_other_contract(data).is_err() {
        // This is okay, the data was given as an explicit argument
        modify_data(data_write_guard);
    }

    Ok(())
}

fn call_into_other_contract(data: &RwLock<Data>) -> Result<(), Error> {
    // Only succeeds if there isn't already write lock acquired
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
        theme-id: 1
        dark-theme-id: 200
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
        theme-id: 1
        dark-theme-id: 200
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

# Contract I/O

Not only contract methods do not have access to general purpose key-value store (even if private to the contract), they
don't have access to any other data except such that was explicitly provided as method input. They also can't return
data in any other way except through return arguments. Execution environment will pre-allocate memory for all
slots/outputs and provide it to the method to work with, removing a need for heap allocation in many cases.

One can think about contract logic as a pure function: it takes inputs and slots, potentially modifies slots and returns
outputs.

Conceptually, all methods look something like this:

```rust,ignore
impl MyContract {
    fn method(
        &self,
        env: &mut Env,
        slot: &MaybeData<u128>,
        input: &Balance,
        output: &mut MaybeData<u128>
    ) -> Result<(), ContractError> {
        if env.context() != &self.owner {
            return Err(ContractError::Forbidden);
        }
        let Some(slot_value) = slot.get().copied() else {
            return Err(ContractError::PreconditionFailed);
        };
        if input > slot_value {
            return Err(ContractError::InvalidInput);
        }

        let num = env.call_other_contract(slot_value)?;
        output.replace(num);

        Ok(())
    }
}
```

Environment handle allows calling other contracts and request ephemeral state, contract slots can be read and written
to, inputs are read-only and outputs are write-only. `&` or `&mut` in Rust limits what can be done with these types,
there is no other implicit "global" way to read or update ephemeral or permanent state of the blockchain.

Handling everything through explicit inputs and outputs results in straightforward implementation, analysis and testing
approach without side effects. In many cases, even heap allocations can be avoided completely, leading to fast and
compact smart contract implementation.
