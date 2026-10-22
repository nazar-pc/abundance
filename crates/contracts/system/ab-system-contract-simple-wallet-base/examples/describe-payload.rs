//! Prints the declared outputs and the methods of a payload of `SimpleWalletBase` as
//! `TransactionPayloadDecoder` describes them, without calling them.
//!
//! The only argument is a file with the bytes of the payload, for example one that the
//! `build-payload` example writes, which shows the commands to run both.
//!
//! The payload is only known when the example runs. So building it with the `no-panic` feature
//! checks that `TransactionPayloadDecoder::new()` and `TransactionPayloadDecoder::next_method()`
//! can't panic, whatever the payload is.

use ab_aligned_buffer::OwnedAlignedBuffer;
use ab_system_contract_simple_wallet_base::MAX_OUTPUTS;
use ab_system_contract_simple_wallet_base::payload::decoder::{
    DecodedInput, DecodedOutput, DecodedSlot, OutputRef, TransactionPayloadDecoder,
};
use std::error::Error;
use std::fmt::Write;
use std::mem::MaybeUninit;
use std::{env, fs};

fn main() -> Result<(), Box<dyn Error>> {
    let path = env::args_os()
        .nth(1)
        .ok_or("Usage: describe-payload <file with the payload>")?;
    let bytes = fs::read(path)?;

    if !bytes.len().is_multiple_of(size_of::<u128>()) {
        return Err("The size of the payload must be a multiple of 16 bytes".into());
    }
    // The decoder requires a payload aligned to 16 bytes
    let payload = OwnedAlignedBuffer::from_bytes(&bytes);

    let mut declared_outputs = [MaybeUninit::uninit(); MAX_OUTPUTS];
    let mut decoder =
        TransactionPayloadDecoder::new(payload.as_u128_slice(), &mut declared_outputs)?;
    for (output_index, declared_output) in decoder.declared_outputs().iter().enumerate() {
        println!(
            "Declared output {output_index}: capacity {}",
            declared_output.capacity
        );
    }
    let mut method_index = 0usize;
    while let Some(method) = decoder.next_method()? {
        println!(
            "Method {method_index}: contract {:?}, fingerprint {}, context {:?}",
            method.contract(),
            method.fingerprint(),
            method.context()
        );
        for (slot_index, slot) in method.slots().iter().enumerate() {
            match slot {
                DecodedSlot::Address(address) => {
                    println!("    slot {slot_index}: {address:?}");
                }
                DecodedSlot::Output(output) => {
                    println!("    slot {slot_index}: {}", describe_output_ref(output));
                }
            }
        }
        for (input_index, input) in method.inputs().iter().enumerate() {
            match input {
                DecodedInput::Value { bytes, alignment } => {
                    println!(
                        "    input {input_index}: {} bytes aligned to {}: {}",
                        bytes.len(),
                        alignment.get(),
                        hex(bytes)
                    );
                }
                DecodedInput::Output(output) => {
                    println!("    input {input_index}: {}", describe_output_ref(output));
                }
            }
        }
        for output in method.outputs() {
            println!("    {}", describe_output(output));
        }

        method_index += 1;
    }

    Ok(())
}

/// Description of the value that an earlier method wrote into a declared output
fn describe_output_ref(output: &OutputRef) -> String {
    match output.return_value_size() {
        Some(size) => format!(
            "return value of {size} bytes with output index {}",
            output.output_index
        ),
        None => format!("`#[output]` with output index {}", output.output_index),
    }
}

/// Description of an output of the method
fn describe_output(output: &DecodedOutput) -> String {
    match output.return_value_size() {
        Some(size) => format!(
            "return value of {size} bytes with output index {}, capacity {} bytes",
            output.output_index, output.capacity
        ),
        None => format!(
            "`#[output]` with output index {}, capacity {} bytes",
            output.output_index, output.capacity
        ),
    }
}

/// Bytes in hexadecimal
fn hex(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(hex, "{byte:02x}").expect("Writing into a `String` doesn't fail; qed");
    }

    hex
}
