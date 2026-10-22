//! What only the caller checks, because it depends on calling methods, and how it allocates outputs

use crate::payload::test_utils::{CallWith, OutputTag, SlotTag, TestPayload, call_all, decode_all};
use crate::payload::{
    FfiDataSizeCapacityRw, TransactionPayloadCaller, TransactionPayloadDecoderError,
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
    payload.method(&[], &[], 1).output(
        Address::SIZE,
        OutputTag::output(Alignment::of::<Address>().get()),
    );
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
            .output(capacity, OutputTag::output(1));
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
fn payload_call_outputs_aligned_without_padding() {
    let mut payload = TestPayload::new();
    // Two outputs with capacity 16 and an alignment of `Alignment::MAX`
    payload
        .method(&[], &[], 2)
        .output(16, OutputTag::output(Alignment::MAX.get()))
        .output(16, OutputTag::output(Alignment::MAX.get()));
    let payload = payload.as_u128_slice();

    let mut external_args_buffer = [ptr::null_mut(); EXTERNAL_ARGS_BUFFER_SIZE];
    // Exactly the capacity of both outputs
    let mut output_buffer = [MaybeUninit::uninit(); 2];
    let output_buffer_addr = output_buffer.as_ptr().addr();
    let mut output_buffer_details = [MaybeUninit::uninit(); OUTPUT_BUFFER_OFFSETS_SIZE];

    let caller = TransactionPayloadCaller::new(
        payload,
        &mut external_args_buffer,
        &mut output_buffer,
        &mut output_buffer_details,
        |_method_context| MethodContext::Reset,
    );

    // The method is only decoded, its outputs keep size `0`
    caller
        .call_next_method_with(&CallWith::new(|method: PreparedMethod<'_>| {
            // SAFETY: The method has two outputs
            let [first_output, second_output] = unsafe {
                method
                    .external_args
                    .cast::<[FfiDataSizeCapacityRw; 2]>()
                    .read()
            };

            // Outputs whose offset is already aligned are not padded
            assert_eq!(first_output.data_ptr.addr(), output_buffer_addr);
            assert_eq!(second_output.data_ptr.addr(), output_buffer_addr + 16);

            Ok(())
        }))
        .unwrap()
        .unwrap();
}

#[test]
fn payload_call_outputs_padded_to_alignment() {
    let mut payload = TestPayload::new();
    // Three outputs with alignments 1, 4 and `Alignment::MAX` and capacities equal to their
    // alignments
    payload
        .method(&[], &[], 3)
        .output(1, OutputTag::output(1))
        .output(4, OutputTag::output(4))
        .output(16, OutputTag::output(Alignment::MAX.get()));
    let payload = payload.as_u128_slice();

    let mut external_args_buffer = [ptr::null_mut(); EXTERNAL_ARGS_BUFFER_SIZE];
    // Exactly the capacity of the outputs with the padding before the second and the third one
    let mut output_buffer = [MaybeUninit::uninit(); 2];
    let output_buffer_addr = output_buffer.as_ptr().addr();
    let mut output_buffer_details = [MaybeUninit::uninit(); OUTPUT_BUFFER_OFFSETS_SIZE];

    let caller = TransactionPayloadCaller::new(
        payload,
        &mut external_args_buffer,
        &mut output_buffer,
        &mut output_buffer_details,
        |_method_context| MethodContext::Reset,
    );

    // The method is only decoded, its outputs keep size `0`
    caller
        .call_next_method_with(&CallWith::new(|method: PreparedMethod<'_>| {
            // SAFETY: The method has three outputs
            let outputs = unsafe {
                method
                    .external_args
                    .cast::<[FfiDataSizeCapacityRw; 3]>()
                    .read()
            };

            // Each output starts at the next offset aligned as declared after the previous one
            let offsets = outputs.map(|output| output.data_ptr.addr() - output_buffer_addr);
            assert_eq!(offsets, [0, 4, 16]);

            Ok(())
        }))
        .unwrap()
        .unwrap();
}
