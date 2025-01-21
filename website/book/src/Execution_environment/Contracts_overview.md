# Everything is a contract

In contrast to many blockchains, the address is just a monotonically increasing number, decoupled from public key (in
case of end user wallet) or code (in case of what is typically understood as "smart contract" in other blockchains).

The address is allocated on account creation and doesn't change regardless of how the contract evolves in the future.
This means that externally all contracts essentially look the same regardless of what they represent. This not only
includes contracts created/deployed by users/developers, but also some fundamental blockchain features.

A few examples of contracts:

* a wallet (can be something simple that only checks signature or a complex smart wallet with multisig/2FA)
* utility functions that offer some shared logic like exotic signature verification
* various kinds of tokens, including native token of the blockchain itself
* even fundamental pieces of logic that allocate addresses and deploy other contracts are contracts themselves

It'll be clear later how far this concept can be stretched, but so far the potential is quite high to make as many
things as possible "just a contract".

This helps to reduce number of special cases for built-in functions vs something that blockchain user can deploy.

# Storage model

In contrast to most other blockchains, contract's storage is organized into a container that has slots inside, which
form a tree with the root being the root of contract's storage.

Each slot belongs to one of the existing contracts and can only be modified by that contract. Contract's code and state
are also slots belonging to system contracts, even though developer-facing API might abstract it in a more friendly way.

Visually it looks something like this:

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

Contracts do not have access to underlying storage implementation in form of key-value database, instead they modify
slots as the only way of persisting data.

# Transaction processing

Each method call of the contract includes metadata about what slots it will read or modify alongside with any inputs or
outputs it expects and their type information. With this information contract execution engine can run non-conflicting
transactions in parallel.

Not only that, it can follow the chain of calls ensuring Rust-like ownership model where contract can't recursively call
its own method that mutates slots because it'll violate safety invariants. Recursive calls of stateless or read-only
methods is fine though.

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

Such loop will be caught and transaction will be aborted:

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
don't have access to any other data except such that was explicitly provided as method input and can't return data in
any other way except through return arguments.

Conceptually all methods look something like this:

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
            return Err(ErrorCode::AccessDenied);
        }
        let Some(slot_value) = slot.get().copied() else {
            return Err(ErrorCode::InvalidState);
        };
        if input > slot_value {
            return Err(ErrorCode::InvalidInput);
        }

        let num = env.call_other_contract(slot_value)?;
        output.replace(num);

        Ok(())
    }
}
```

Environment handle allows to call other contracts and request ephemeral state, contract's slots can be read and written
to, inputs are read-only and outputs are write-only. `&` or `&mut` in Rust limits what can be done with these types,
there is no other implicit  "global" way to read or update ephemeral or permanent state of the blockchain.

Handling everything through explicit inputs and outputs results in straightforward implementation, analysis and testing
approach without side effects. In many cases even heap allocations can be avoided completely, leading to fast and
compact smart contract implementation.
