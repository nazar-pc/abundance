//! What only the caller checks, because it depends on calling methods, and how it allocates outputs

use crate::payload::test_utils::{CallWith, OutputTag, SlotTag, TestPayload, call_all, decode_all};
use crate::payload::{
    FfiDataSizeCapacityRw, TransactionPayloadCallError, TransactionPayloadCaller,
    TransactionPayloadDecoderError,
};
use crate::{EXTERNAL_ARGS_BUFFER_SIZE, OUTPUT_BUFFER_OFFSETS_SIZE, OUTPUT_BUFFER_SIZE};
use ab_contracts_common::env::{MethodContext, PreparedMethod};
use ab_core_primitives::address::Address;
use ab_io_type::alignment::Alignment;
use ab_io_type::trivial_type::TrivialType;
use core::mem::MaybeUninit;
use core::ptr;

#[test]
fn payload_call_slot_output_size() {
    // An `#[output]` used as a slot, whose size is only known after its method was called
    let mut payload = TestPayload::new();
    payload
        .method(&[], &[], 1)
        .output(Address::SIZE, OutputTag::output());
    payload.method(&[SlotTag::output_index(0)], &[], 0);
    let payload = payload.as_u128_slice();

    // The decoder accepts it, and the caller rejects it when the method leaves it empty
    assert_eq!(decode_all(payload, OUTPUT_BUFFER_OFFSETS_SIZE), Ok(2));
    let mut output_buffer = [MaybeUninit::uninit(); OUTPUT_BUFFER_SIZE];
    assert_eq!(
        call_all(payload, &mut output_buffer, OUTPUT_BUFFER_OFFSETS_SIZE),
        Err(TransactionPayloadDecoderError::InvalidSlotOutputIndexSize {
            output_index: 0,
            size: 0,
        })
    );

    // The caller accepts it when the method writes an address into it
    let address = Address::SYSTEM_STATE;
    let mut external_args_buffer = [ptr::null_mut(); EXTERNAL_ARGS_BUFFER_SIZE];
    let mut output_buffer_details = [MaybeUninit::uninit(); OUTPUT_BUFFER_OFFSETS_SIZE];
    let caller = TransactionPayloadCaller::new(
        payload,
        &mut external_args_buffer,
        &mut output_buffer,
        &mut output_buffer_details,
        |_method_context| MethodContext::Reset,
    );
    let caller = caller
        .call_next_method_with(&CallWith::new(|method: PreparedMethod<'_>| {
            // SAFETY: The method has a single `#[output]` argument
            let output = unsafe {
                method
                    .external_args
                    .cast::<FfiDataSizeCapacityRw>()
                    .as_mut()
            };
            assert_eq!(output.size, 0);
            assert_eq!(output.capacity, Address::SIZE);
            // SAFETY: Output buffer has the requested capacity
            unsafe {
                output
                    .data_ptr
                    .copy_from_nonoverlapping(address.as_bytes().as_ptr(), size_of::<Address>());
            }
            output.size = Address::SIZE;

            Ok(())
        }))
        .unwrap()
        .unwrap();
    let caller = caller
        .call_next_method_with(&CallWith::new(|method: PreparedMethod<'_>| {
            // SAFETY: The method has a single `#[slot]` argument, which points to the `#[output]`
            // of the previous method
            let slot = unsafe { method.external_args.cast::<*const Address>().read().read() };
            assert_eq!(slot, address);

            Ok(())
        }))
        .unwrap()
        .unwrap();
    assert!(
        caller
            .call_next_method_with(&CallWith::new(|_method: PreparedMethod<'_>| {
                unreachable!("There are no more methods")
            }))
            .unwrap()
            .is_none()
    );
}

#[test]
fn payload_call_output_buffer_too_small() {
    // The output buffer belongs to the caller, so only the caller checks that outputs fit into it
    let output_buffer_size = u32::try_from(OUTPUT_BUFFER_SIZE * size_of::<u128>()).unwrap();
    for (capacity, expected) in [
        (output_buffer_size, Ok(1)),
        (
            output_buffer_size + 1,
            Err(TransactionPayloadDecoderError::OutputBufferTooSmall),
        ),
    ] {
        let mut payload = TestPayload::new();
        payload
            .method(&[], &[], 1)
            .output(capacity, OutputTag::output());
        let payload = payload.as_u128_slice();

        assert_eq!(
            decode_all(payload, OUTPUT_BUFFER_OFFSETS_SIZE),
            Ok(1),
            "Capacity {capacity}"
        );
        let mut output_buffer = [MaybeUninit::uninit(); OUTPUT_BUFFER_SIZE];
        assert_eq!(
            call_all(payload, &mut output_buffer, OUTPUT_BUFFER_OFFSETS_SIZE),
            expected,
            "Capacity {capacity}"
        );
    }
}

#[test]
fn payload_call_outputs_at_max_alignment() {
    let mut payload = TestPayload::new();
    // Three outputs with capacities 1, 4 and 16
    payload
        .method(&[], &[], 3)
        .output(1, OutputTag::output())
        .output(4, OutputTag::output())
        .output(16, OutputTag::output());
    let payload = payload.as_u128_slice();

    // Each output starts at the next multiple of `Alignment::MAX` after the previous one, and the
    // third one isn't padded, so they need exactly 48 bytes: they fit into three `u128`s, but not
    // into two
    for (output_buffer_size, fits) in [(3, true), (2, false)] {
        let mut output_buffer = [MaybeUninit::uninit(); 3];
        let output_buffer = &mut output_buffer[..output_buffer_size];
        let output_buffer_addr = output_buffer.as_ptr().addr();
        let mut external_args_buffer = [ptr::null_mut(); EXTERNAL_ARGS_BUFFER_SIZE];
        let mut output_buffer_details = [MaybeUninit::uninit(); OUTPUT_BUFFER_OFFSETS_SIZE];

        let caller = TransactionPayloadCaller::new(
            payload,
            &mut external_args_buffer,
            output_buffer,
            &mut output_buffer_details,
            |_method_context| MethodContext::Reset,
        );

        // The method is only decoded, its outputs keep size `0`
        let result = caller
            .call_next_method_with(&CallWith::new(|method: PreparedMethod<'_>| {
                // SAFETY: The method has three outputs
                let outputs = unsafe {
                    method
                        .external_args
                        .cast::<[FfiDataSizeCapacityRw; 3]>()
                        .read()
                };

                let offsets = outputs.map(|output| output.data_ptr.addr() - output_buffer_addr);
                assert_eq!(offsets, [0, 16, 32]);

                Ok(())
            }))
            .map(|maybe_caller| maybe_caller.is_some());
        if fits {
            assert!(result.unwrap(), "{output_buffer_size} `u128`s");
        } else {
            assert!(
                matches!(
                    result,
                    Err(TransactionPayloadCallError::Decoder(
                        TransactionPayloadDecoderError::OutputBufferTooSmall
                    ))
                ),
                "{output_buffer_size} `u128`s"
            );
        }
    }
}

#[test]
fn payload_call_slot_from_output_aligned() {
    // A return value follows an `#[output]` with capacity `1`, so its memory would be misaligned
    // for an address if it was allocated right after it
    let mut payload = TestPayload::new();
    payload
        .method(&[], &[], 2)
        .output(1, OutputTag::output())
        .output(Address::SIZE, OutputTag::return_value());
    payload.method(&[SlotTag::output_index(1)], &[], 0);
    let payload = payload.as_u128_slice();

    let mut external_args_buffer = [ptr::null_mut(); EXTERNAL_ARGS_BUFFER_SIZE];
    // Zeroed, so the return value that the first method doesn't write is initialized
    let mut output_buffer = [MaybeUninit::new(0); OUTPUT_BUFFER_SIZE];
    let mut output_buffer_details = [MaybeUninit::uninit(); OUTPUT_BUFFER_OFFSETS_SIZE];
    let caller = TransactionPayloadCaller::new(
        payload,
        &mut external_args_buffer,
        &mut output_buffer,
        &mut output_buffer_details,
        |_method_context| MethodContext::Reset,
    );
    let caller = caller
        .call_next_method_with(&CallWith::new(|_method: PreparedMethod<'_>| Ok(())))
        .unwrap()
        .unwrap();
    caller
        .call_next_method_with(&CallWith::new(|method: PreparedMethod<'_>| {
            // SAFETY: The method has a single `#[slot]` argument
            let slot = unsafe { method.external_args.cast::<*const Address>().read() };
            assert_eq!(slot.addr() % usize::from(Alignment::MAX.get()), 0);

            Ok(())
        }))
        .unwrap()
        .unwrap();
}
