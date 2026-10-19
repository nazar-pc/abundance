//! Builds a payload of `SimpleWalletBase` with `TransactionPayloadBuilder` and writes its bytes
//! into a file, which the `describe-payload` example reads:
//!
//! ```text
//! cargo run -p ab-system-contract-simple-wallet-base --features payload-builder \
//!     --example build-payload -- payload.bin
//! cargo run -p ab-system-contract-simple-wallet-base --example describe-payload -- payload.bin
//! ```
//!
//! The payload moves the whole native token balance of a wallet to another address. It declares one
//! output, the first method returns the balance into it, and the second one transfers it, taking
//! the amount from that output.

use ab_contracts_standards::fungible::ffi::balance::FungibleBalanceArgs;
use ab_contracts_standards::fungible::ffi::transfer::FungibleTransferArgs;
use ab_core_primitives::address::Address;
use ab_core_primitives::balance::Balance;
use ab_system_contract_simple_wallet_base::payload::TransactionMethodContext;
use ab_system_contract_simple_wallet_base::payload::builder::{
    MethodCallReferences, TransactionPayloadBuilder,
};
use std::error::Error;
use std::mem::MaybeUninit;
use std::{env, fs};

fn main() -> Result<(), Box<dyn Error>> {
    let path = env::args_os()
        .nth(1)
        .ok_or("Usage: build-payload <file to write the payload to>")?;

    let wallet = Address::from(1000);
    let recipient = Address::from(1001);

    // The only declared output has output index 0, the builder computes its capacity
    let mut builder = TransactionPayloadBuilder::new(1024, 1)?;
    // The balance that this method returns is written into output index 0
    builder.with_method_call(
        &Address::SYSTEM_NATIVE_TOKEN,
        &FungibleBalanceArgs::new(&wallet, &mut MaybeUninit::uninit()),
        TransactionMethodContext::Null,
        MethodCallReferences {
            outputs: &[0],
            ..MethodCallReferences::default()
        },
    )?;
    // The amount comes from output index 0, so the amount given here is not in the payload
    builder.with_method_call(
        &Address::SYSTEM_NATIVE_TOKEN,
        &FungibleTransferArgs::new(&wallet, &recipient, &Balance::MIN),
        TransactionMethodContext::Wallet,
        MethodCallReferences {
            inputs: &[None, None, Some(0)],
            ..MethodCallReferences::default()
        },
    )?;

    fs::write(path, builder.into_aligned_bytes()?.as_slice())?;

    Ok(())
}
