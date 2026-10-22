//! How the caller allocates outputs

use crate::payload::test_utils::{CallWith, NUM_SLOT_ARGUMENTS_OFFSET};
use crate::payload::{FfiDataSizeCapacityRw, TransactionPayloadCaller};
use crate::{EXTERNAL_ARGS_BUFFER_SIZE, OUTPUT_BUFFER_OFFSETS_SIZE};
use ab_contracts_common::env::{MethodContext, PreparedMethod};
use ab_io_type::alignment::Alignment;
use core::mem::MaybeUninit;
use core::ptr;

#[test]
fn payload_call_outputs_aligned_without_padding() {
    let mut payload = [0u8; 80];
    // No slots and inputs, two outputs (the first bit) with capacity 16 and an alignment of
    // `Alignment::MAX` (power of two in the remaining bits)
    payload[NUM_SLOT_ARGUMENTS_OFFSET + 2] = 2;
    let mut capacity_offset = NUM_SLOT_ARGUMENTS_OFFSET + 3;
    for _ in 0..2 {
        capacity_offset = capacity_offset.next_multiple_of(align_of::<u32>());
        payload[capacity_offset..][..size_of::<u32>()].copy_from_slice(&16u32.to_le_bytes());
        payload[capacity_offset + size_of::<u32>()] = 0b1000_0000 | Alignment::MAX.power;
        capacity_offset += size_of::<u32>() + 1;
    }
    let payload = core::array::from_fn::<_, 5, _>(|index| {
        u128::from_ne_bytes(payload.as_chunks::<{ size_of::<u128>() }>().0[index])
    });

    let mut external_args_buffer = [ptr::null_mut(); EXTERNAL_ARGS_BUFFER_SIZE];
    // Exactly the capacity of both outputs
    let mut output_buffer = [MaybeUninit::uninit(); 2];
    let output_buffer_addr = output_buffer.as_ptr().addr();
    let mut output_buffer_details = [MaybeUninit::uninit(); OUTPUT_BUFFER_OFFSETS_SIZE];

    let caller = TransactionPayloadCaller::new(
        &payload,
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
    let mut payload = [0u8; 80];
    // No slots and inputs, three outputs (the first bit) with alignments 1, 4 and 16 (power of two
    // in the remaining bits) and capacities equal to their alignments
    payload[NUM_SLOT_ARGUMENTS_OFFSET + 2] = 3;
    let mut capacity_offset = NUM_SLOT_ARGUMENTS_OFFSET + 3;
    for alignment_power in [0, 2, Alignment::MAX.power] {
        capacity_offset = capacity_offset.next_multiple_of(align_of::<u32>());
        payload[capacity_offset..][..size_of::<u32>()]
            .copy_from_slice(&(1u32 << alignment_power).to_le_bytes());
        payload[capacity_offset + size_of::<u32>()] = 0b1000_0000 | alignment_power;
        capacity_offset += size_of::<u32>() + 1;
    }
    let payload = core::array::from_fn::<_, 5, _>(|index| {
        u128::from_ne_bytes(payload.as_chunks::<{ size_of::<u128>() }>().0[index])
    });

    let mut external_args_buffer = [ptr::null_mut(); EXTERNAL_ARGS_BUFFER_SIZE];
    // Exactly the capacity of the outputs with the padding before the second and the third one
    let mut output_buffer = [MaybeUninit::uninit(); 2];
    let output_buffer_addr = output_buffer.as_ptr().addr();
    let mut output_buffer_details = [MaybeUninit::uninit(); OUTPUT_BUFFER_OFFSETS_SIZE];

    let caller = TransactionPayloadCaller::new(
        &payload,
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
