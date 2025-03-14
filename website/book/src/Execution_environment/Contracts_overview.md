# Everything is a contract

In contrast to many blockchains, the address is just a monotonically increasing number, decoupled from a public key (in
case of end user wallet) or code (in case of what is typically understood as "smart contract" in other blockchains).

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
    fn method(
        &self,
        #[env] env: &mut Env,
        #[slot] slot: &MaybeData<u128>,
        #[input] input: &Balance,
        #[output] output: &mut MaybeData<u128>
    ) -> Result<(), ContractError> {
        if env.context() != &self.owner {
            return Err(ContractError::Forbidden);
        }
        let Some(slot_value) = slot.get().copied() else {
            return Err(ContractError::BadInput);
        };
        if input > slot_value {
            return Err(ContractError::BadInput);
        }

        let num = env.call_other_contract(slot_value)?;
        output.replace(num);

        Ok(())
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
