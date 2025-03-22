# Everything is a contract

Every contract has an address, which is just a monotonically increasing number. This is in contract to many blockchains
where address might be derived from a public key (in case of end user wallet) or code (of "smart contracts"). There is
no separate notion of Externally Owned Account (EOA) like in Ethereum, end user wallets are also just contracts.

The address is allocated on contract creation and doesn't change regardless of how the contract evolves in the future.
This means that externally, all contracts essentially look the same regardless of what they represent. This enables a
wallet contract to change its logic from verifying a single signature to multisig to 2FA to use a completely different
cryptography in the future, all while retaining its address/identity.

This not only includes contracts created/deployed by users/developers, but also some fundamental blockchain features.
For example, in most blockchains code of the contract is stored in a special location by the node and retrieved before
processing a [transaction]. Here code is managed by a system `code` contract instead of the node as such, and deployment
of a new contract is a call to system `code` contract instead of special host function provided by the node and `code`
contract will store the code in the corresponding slot of the newly created contract (see [Storage model] below for more
details).

[transaction]: Transactions_overview#transactions-abstraction

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

[Transaction processing]: Transactions_overview#transaction-processing

Each slot is managed by exactly one of the existing contracts and can only be read or modified by that contract.
Contract's code and state are also slots managed by contracts (system contracts), even though developer-facing API might
abstract it in a more friendly way. It is possible for a contract to manage one of its slots too, like when a token
contract owns some number of its own tokens.

A bit wrong, but hopefully useful analogy is cloud server. A server is owned by a provider, but managed by a customer.
Provider typically doesn't have remote access to the server customer orders, all changes to the software that server
runs are done by the customer. Similarly, slots owned by a contract are managed by other contracts.

In contrast to most other blockchains by "state" we refer to the inherent state of the contract itself, rather than
things that might belong to end-users. The right mental model is to think of it as a global state of a contract.

Let's take a generic fungible token as an example. System `state` contract will manage its state, stored in
corresponding slot owned by the token contract. State will contain things like total supply and potentially useful
metadata like number of decimal places and ticker, but not balances of individual users.

The state of the contract (and any other slot) is typically bounded in size and defined by contract developer upfront.
Bounded size allows execution environment to allocate the necessary amount of memory and to limit the amount of data
that potentially needs to be sent with the transaction over the network (see [Transaction processing]).

This implies there can't be more traditional unbounded hashmap there. Instead, balances are stored in slots of contracts
that own the balance (like smart wallet owned by end user), but managed by the token contract. This is similar to how
contract's state and code are managed by corresponding system contracts.

Visually, it looks something like this:

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

# Contract I/O

Not only contract methods do not have access to general purpose key-value store (even if private to the contract), they
don't have access to any other data except such that was explicitly provided as method input. They also can't return
data in any other way except through return arguments. Execution environment will pre-allocate memory for all
slots/outputs and provide it to the method to work with, removing a need for heap allocation in many cases.

One can think about contract logic as a pure function: it takes inputs and slots, potentially modifies slots and returns
outputs.

Conceptually, all methods look something like this:

```rust,ignore
#[contract]
impl MyContract {
    /// Stateless compute
    #[view]
    pub fn add(
        #[input] &a: &u32,
        #[input] &b: &u32,
    ) -> u32 {
        a + b
    }

    /// Calling another contract (in this case into itself)
    #[view]
    pub fn add_through_contract_call(
        #[env] env: &Env,
        #[input] &a: &u32,
        #[input] &b: &u32,
    ) -> Result<u32, ContractError> {
        env.my_contract_add(env.own_address(), a, b)
    }

    /// Modifying its own state using the contents of the slot
    #[update]
    pub fn self_increment_by_slot(
        &mut self,
        #[slot] slot: &MaybeData<u32>,
    ) -> Result<u32, ContractError> {
        let old_value = self.value;

        // The slot may or may not exist yet
        let Some(slot_value) = slot.get().copied() else {
            return Err(ContractError::Forbidden);
        };

        self.value = old_value
            .checked_add(slot_value)
            .ok_or(ContractError::BadInput)?;
        Ok(old_value)
    }
}
```

Environment handle allows calling other contracts and request ephemeral state, contract slots can be read and written
to, inputs are read-only and outputs are write-only. `&` or `&mut` in Rust limits what can be done with these types,
there is no other implicit "global" way to read or update ephemeral or permanent state of the blockchain except through
these explicit arguments.

Handling everything through explicit inputs and outputs results in straightforward implementation, analysis and testing
approach without side effects. In many cases, even heap allocations can be avoided completely, leading to fast and
compact smart contract implementation.

`#[contract]` macro and attributes like `#[env]` do not impact the code in a method in any way, but help to generate
additional helper data structures, functions and metadata about the contract. The macro also verifies a lot of different
invariants about the contract with helpful compile-time error messages if something goes wrong. For example, when
different methods use different types for `#[slot]` argument or when type not allowed for FFI is used in an argument.

# Method call context

When calling into another contract, a method context needs to be specified. The correct mental model for context is
"user of the child process," where "process" is a method call. Essentially, something executed with a context of a
contract can be thought as done "on behalf" of that contract, which depending on circumstances may or may not be
desired.

Initially, context is "Null." For each call into another contract, the context of the current method can be either
preserved, reset to "Null" or replaced with the current contract's address. Those are the only options. Contracts do not
have privileges to change context to the address of an arbitrary contract.

The safest option is to reset context to "Null," which means called contract will be able to "know" who called it, but
unable to convince any further calls in it. Preservation of the context allows to "delegate" certain operations to
another contract, which while is potentially dangerous, allows for more advanced use cases.

In addition to argument attributes mentioned before, there is also `#[tmp]`, which is essentially an ephemeral storage
that only lives for the duration of a single [transaction]. It can be used for temporary approvals, allowing to use
"Null" context for most operations, while also allowing for contracts to do certain operations effectively on behalf of
the caller. For example, in a transaction, the first call might approve a defi contract to spend some tokens and then
call defi contract to actually do the operation. Both calls are done with "Null" method context, but still achieve the
desired impact with the least permission possible.

# Metadata

Metadata about contract is a crucial piece used in many places of the system. Metadata essentially describes in a
compact binary format all traits and methods that the contract implements alongside recursive metadata of all types in
those methods.

Metadata contains exhaustive details about the method, allowing execution environment to encode and decode arguments
from/to method calls from metadata alone.

Metadata can also be used to auto-generate user-facing interfaces and FFI bindings to other languages since it contains
relatively basic types.

The same metadata can also be used by a transaction handler contract to encode/decode method calls in a [transaction].
This is huge because with metadata being an inherent part of the contract itself, enables hardware wallets to accurately
and verifiably render transaction contents in a relatively user-friendly way, especially on those with larger screens.
This means no blind signing anymore and no need to trust the computer wallet is connected to either, the wallet can
verify and render everything on its own.
