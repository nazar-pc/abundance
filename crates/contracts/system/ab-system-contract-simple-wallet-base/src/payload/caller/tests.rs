//! What only the caller checks, because it depends on calling methods, and how it allocates outputs

use crate::payload::caller::{TransactionPayloadCallError, TransactionPayloadCaller};
use crate::payload::test_utils::{
    CallWith, InputTag, OUTPUT_BUFFER_BYTES, OutputTag, SlotTag, TestPayload,
    assert_no_more_methods, call_all, call_next, decode_all, new_caller, only_input, only_output,
    only_slot,
};
use crate::payload::{FfiDataSizeCapacityRw, TransactionPayloadDecoderError};
use crate::{EXTERNAL_ARGS_BUFFER_SIZE, MAX_OUTPUTS, OUTPUT_BUFFER_SIZE};
use ab_contracts_common::env::{MethodContext, PreparedMethod};
use ab_core_primitives::address::Address;
use ab_io_type::alignment::Alignment;
use ab_io_type::trivial_type::TrivialType;
use core::mem::MaybeUninit;
use core::ptr;

#[test]
fn payload_call_slot_output_size() {
    // An `#[output]` used as a slot, whose size is only known after its method was called
    let mut payload = TestPayload::new(&[Address::SIZE]);
    payload.method(&[], &[], &[OutputTag::output(0)]);
    payload.method(&[SlotTag::output_index(0)], &[], &[]);
    let payload = payload.as_u128_slice();

    // The decoder accepts it, and the caller rejects it when the method leaves it empty
    assert_eq!(decode_all(payload), Ok(2));
    let mut output_buffer = [MaybeUninit::uninit(); OUTPUT_BUFFER_SIZE];
    assert_eq!(
        call_all(payload, &mut output_buffer),
        Err(TransactionPayloadDecoderError::InvalidSlotOutputIndexSize {
            output_index: 0,
            size: 0,
        })
    );

    // The caller accepts it when the method writes an address into it
    let address = Address::SYSTEM_STATE;
    let mut external_args_buffer = [ptr::null_mut(); EXTERNAL_ARGS_BUFFER_SIZE];
    let mut declared_outputs = [MaybeUninit::uninit(); MAX_OUTPUTS];
    let caller = new_caller(
        payload,
        &mut external_args_buffer,
        &mut output_buffer,
        &mut declared_outputs,
    );
    let caller = call_next(caller, |method: PreparedMethod<'_>| {
        // SAFETY: The method has a single `#[output]` argument
        let output = unsafe { only_output(&method) };
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
    });
    let caller = call_next(caller, |method: PreparedMethod<'_>| {
        // SAFETY: The method has a single `#[slot]` argument, which points to the `#[output]` of
        // the previous method
        let slot = unsafe { only_slot(&method).read() };
        assert_eq!(slot, address);

        Ok(())
    });
    assert_no_more_methods(caller);
}

#[test]
fn payload_call_output_buffer_too_small() {
    // The output buffer belongs to the caller, so only the caller checks that the declared
    // outputs fit into it. Each of them starts at a multiple of `Alignment::MAX`.
    for (capacities, fits) in [
        (&[OUTPUT_BUFFER_BYTES][..], true),
        (&[OUTPUT_BUFFER_BYTES + 1], false),
        (&[OUTPUT_BUFFER_BYTES - 16, 16], true),
        (&[OUTPUT_BUFFER_BYTES - 16, 17], false),
        // The second one starts at 16, so without rounding it would fit
        (&[1, OUTPUT_BUFFER_BYTES - 16], true),
        (&[1, OUTPUT_BUFFER_BYTES - 15], false),
        (&[OUTPUT_BUFFER_BYTES, 0], true),
    ] {
        // The output that doesn't fit is only written by the second method, the first one has
        // no arguments
        let last_output_index = u8::try_from(capacities.len() - 1).unwrap();
        let mut payload = TestPayload::new(capacities);
        payload.method(&[], &[], &[]);
        payload.method(&[], &[], &[OutputTag::output(last_output_index)]);
        let payload = payload.as_u128_slice();

        assert_eq!(decode_all(payload), Ok(2), "Capacities {capacities:?}");
        // The caller rejects the payload when it is created, before it calls any method
        let mut external_args_buffer = [ptr::null_mut(); EXTERNAL_ARGS_BUFFER_SIZE];
        let mut output_buffer = [MaybeUninit::uninit(); OUTPUT_BUFFER_SIZE];
        let mut declared_outputs = [MaybeUninit::uninit(); MAX_OUTPUTS];
        let result = TransactionPayloadCaller::new(
            payload,
            &mut external_args_buffer,
            &mut output_buffer,
            &mut declared_outputs,
            |_method_context| MethodContext::Reset,
        );
        if fits {
            assert!(result.is_ok(), "Capacities {capacities:?}");
        } else {
            assert!(
                matches!(
                    result,
                    Err(TransactionPayloadDecoderError::OutputBufferTooSmall)
                ),
                "Capacities {capacities:?}"
            );
        }
    }
}

#[test]
fn payload_call_outputs_at_max_alignment() {
    // Declared outputs with capacities 1, 17 and 0 are allocated at offsets 0, 16 and 48, so the
    // last one is at the end of an output buffer of 48 bytes
    let mut payload = TestPayload::new(&[1, 17, 0]);
    payload.method(
        &[],
        &[],
        &[
            OutputTag::output(0),
            OutputTag::output(1),
            OutputTag::output(2),
        ],
    );
    let payload = payload.as_u128_slice();

    let mut external_args_buffer = [ptr::null_mut(); EXTERNAL_ARGS_BUFFER_SIZE];
    let mut output_buffer = [MaybeUninit::uninit(); 3];
    let output_buffer_addr = output_buffer.as_ptr().addr();
    let mut declared_outputs = [MaybeUninit::uninit(); MAX_OUTPUTS];
    let caller = TransactionPayloadCaller::new(
        payload,
        &mut external_args_buffer,
        &mut output_buffer,
        &mut declared_outputs,
        |_method_context| MethodContext::Reset,
    )
    .unwrap();
    let caller = call_next(caller, |method: PreparedMethod<'_>| {
        // SAFETY: The method has three `#[output]` arguments
        let outputs = unsafe {
            method
                .external_args
                .cast::<[FfiDataSizeCapacityRw; 3]>()
                .read()
        };
        for (output, (offset, capacity)) in outputs.iter().zip([(0, 1), (16, 17), (48, 0)]) {
            assert_eq!(output.data_ptr.addr(), output_buffer_addr + offset);
            assert_eq!(output.size, 0);
            assert_eq!(output.capacity, capacity);
        }

        Ok(())
    });
    assert_no_more_methods(caller);

    // Two declared outputs with capacity `1` need 17 bytes
    let mut payload = TestPayload::new(&[1, 1]);
    payload.method(&[], &[], &[OutputTag::output(0), OutputTag::output(1)]);
    let payload = payload.as_u128_slice();
    for (output_buffer_size, fits) in [(2, true), (1, false)] {
        let mut external_args_buffer = [ptr::null_mut(); EXTERNAL_ARGS_BUFFER_SIZE];
        let mut output_buffer = [MaybeUninit::uninit(); 2];
        let mut declared_outputs = [MaybeUninit::uninit(); MAX_OUTPUTS];
        let result = TransactionPayloadCaller::new(
            payload,
            &mut external_args_buffer,
            &mut output_buffer[..output_buffer_size],
            &mut declared_outputs,
            |_method_context| MethodContext::Reset,
        );
        assert_eq!(result.is_ok(), fits, "{output_buffer_size} chunks");
    }
}

#[test]
fn payload_call_slot_from_output_aligned() {
    // The return value writes the second declared output, which follows one with capacity `1`, so
    // it would be misaligned for an address if declared outputs were allocated right after each
    // other
    let mut payload = TestPayload::new(&[1, Address::SIZE]);
    payload
        .method(
            &[],
            &[],
            &[OutputTag::output(0), OutputTag::return_value(1)],
        )
        .return_value_size(Address::SIZE);
    payload.method(&[SlotTag::output_index(1)], &[], &[]);
    let payload = payload.as_u128_slice();

    let mut external_args_buffer = [ptr::null_mut(); EXTERNAL_ARGS_BUFFER_SIZE];
    // Zeroed, so the return value that the first method doesn't write is initialized
    let mut output_buffer = [MaybeUninit::zeroed(); OUTPUT_BUFFER_SIZE];
    let mut declared_outputs = [MaybeUninit::uninit(); MAX_OUTPUTS];
    let caller = new_caller(
        payload,
        &mut external_args_buffer,
        &mut output_buffer,
        &mut declared_outputs,
    );
    let caller = call_next(caller, |_method: PreparedMethod<'_>| Ok(()));
    let caller = call_next(caller, |method: PreparedMethod<'_>| {
        // SAFETY: The method has a single `#[slot]` argument
        let slot = unsafe { only_slot(&method) };
        assert_eq!(slot.addr() % usize::from(Alignment::MAX.get()), 0);

        Ok(())
    });
    assert_no_more_methods(caller);
}

#[test]
fn payload_call_output_reused_as_output() {
    // Three methods write the same declared output as `#[output]`, and a fourth one reads it
    let mut payload = TestPayload::new(&[64]);
    for _ in 0..3 {
        payload.method(&[], &[], &[OutputTag::output(0)]);
    }
    payload.method(&[], &[InputTag::output_index(0)], &[]);
    let payload = payload.as_u128_slice();

    // Every method gets the same memory empty with the declared capacity, whatever the method
    // before it wrote, and the last one reads what the third one wrote
    let mut external_args_buffer = [ptr::null_mut(); EXTERNAL_ARGS_BUFFER_SIZE];
    let mut output_buffer = [MaybeUninit::uninit(); OUTPUT_BUFFER_SIZE];
    let output_buffer_addr = output_buffer.as_ptr().addr();
    let mut declared_outputs = [MaybeUninit::uninit(); MAX_OUTPUTS];
    let mut caller = new_caller(
        payload,
        &mut external_args_buffer,
        &mut output_buffer,
        &mut declared_outputs,
    );
    for written in [5, 3, 2] {
        caller = call_next(caller, |method: PreparedMethod<'_>| {
            // SAFETY: The method has a single `#[output]` argument
            let output = unsafe { only_output(&method) };
            assert_eq!(output.data_ptr.addr(), output_buffer_addr);
            assert_eq!(output.size, 0);
            assert_eq!(output.capacity, 64);
            // SAFETY: Output buffer has the requested capacity
            unsafe {
                output.data_ptr.write_bytes(written, usize::from(written));
            }
            output.size = u32::from(written);

            Ok(())
        });
    }
    let caller = call_next(caller, |method: PreparedMethod<'_>| {
        // SAFETY: The method has a single `#[input]` argument
        let input = unsafe { only_input(&method) };
        assert_eq!(input.data_ptr.addr().get(), output_buffer_addr);
        assert_eq!(input.size, 2);
        // SAFETY: The input points to the 2 bytes the previous method wrote
        assert_eq!(unsafe { input.data_ptr.cast::<[u8; 2]>().read() }, [2, 2]);

        Ok(())
    });
    assert_no_more_methods(caller);
}

#[test]
fn payload_call_output_reused_by_return_value() {
    // An `#[output]` writes 40 of 64 bytes, then a return value of an address replaces it, which
    // later methods get as an input and as a slot
    let mut payload = TestPayload::new(&[64]);
    payload.method(&[], &[], &[OutputTag::output(0)]);
    payload
        .method(&[], &[], &[OutputTag::return_value(0)])
        .return_value_size(Address::SIZE);
    payload.method(&[], &[InputTag::output_index(0)], &[]);
    payload.method(&[SlotTag::output_index(0)], &[], &[]);
    let payload = payload.as_u128_slice();

    let address = Address::SYSTEM_STATE;
    let mut external_args_buffer = [ptr::null_mut(); EXTERNAL_ARGS_BUFFER_SIZE];
    let mut output_buffer = [MaybeUninit::uninit(); OUTPUT_BUFFER_SIZE];
    let mut declared_outputs = [MaybeUninit::uninit(); MAX_OUTPUTS];
    let caller = new_caller(
        payload,
        &mut external_args_buffer,
        &mut output_buffer,
        &mut declared_outputs,
    );
    let caller = call_next(caller, |method: PreparedMethod<'_>| {
        // SAFETY: The method has a single `#[output]` argument
        let output = unsafe { only_output(&method) };
        // SAFETY: Output buffer has the requested capacity
        unsafe {
            output.data_ptr.write_bytes(1, 40);
        }
        output.size = 40;

        Ok(())
    });
    let caller = call_next(caller, |method: PreparedMethod<'_>| {
        // SAFETY: The method has a return value, whose pointer the caller follows with its size
        // and capacity
        let return_value = unsafe { only_output(&method) };
        assert_eq!(return_value.size, Address::SIZE);
        assert_eq!(return_value.capacity, 64);
        // SAFETY: Output buffer has the requested capacity
        unsafe {
            return_value
                .data_ptr
                .copy_from_nonoverlapping(address.as_bytes().as_ptr(), size_of::<Address>());
        }

        Ok(())
    });
    let caller = call_next(caller, |method: PreparedMethod<'_>| {
        // SAFETY: The method has a single `#[input]` argument
        let input = unsafe { only_input(&method) };
        assert_eq!(input.size, Address::SIZE);
        // SAFETY: The input points to the return value of the second method
        assert_eq!(unsafe { input.data_ptr.cast::<Address>().read() }, address);

        Ok(())
    });
    let caller = call_next(caller, |method: PreparedMethod<'_>| {
        // SAFETY: The method has a single `#[slot]` argument, which points to the return value of
        // the second method
        assert_eq!(unsafe { only_slot(&method).read() }, address);

        Ok(())
    });
    assert_no_more_methods(caller);
}

#[test]
fn payload_call_return_value_reused_by_output() {
    // A return value of an address, then an `#[output]` that writes 3 bytes into the same declared
    // output, which a later method gets as an input, while a slot from it is rejected
    let mut payload = TestPayload::new(&[Address::SIZE]);
    payload
        .method(&[], &[], &[OutputTag::return_value(0)])
        .return_value_size(Address::SIZE);
    payload.method(&[], &[], &[OutputTag::output(0)]);
    payload.method(&[], &[InputTag::output_index(0)], &[]);
    payload.method(&[SlotTag::output_index(0)], &[], &[]);
    let payload = payload.as_u128_slice();

    // The decoder accepts the slot, since the size of an `#[output]` is only known after its
    // method was called
    assert_eq!(decode_all(payload), Ok(4));

    let mut external_args_buffer = [ptr::null_mut(); EXTERNAL_ARGS_BUFFER_SIZE];
    let mut output_buffer = [MaybeUninit::uninit(); OUTPUT_BUFFER_SIZE];
    let mut declared_outputs = [MaybeUninit::uninit(); MAX_OUTPUTS];
    let caller = new_caller(
        payload,
        &mut external_args_buffer,
        &mut output_buffer,
        &mut declared_outputs,
    );
    let caller = call_next(caller, |method: PreparedMethod<'_>| {
        // SAFETY: The method has a return value, whose pointer the caller follows with its size
        // and capacity
        let return_value = unsafe { only_output(&method) };
        // SAFETY: Output buffer has the requested capacity
        unsafe {
            return_value.data_ptr.copy_from_nonoverlapping(
                Address::SYSTEM_STATE.as_bytes().as_ptr(),
                size_of::<Address>(),
            );
        }

        Ok(())
    });
    let caller = call_next(caller, |method: PreparedMethod<'_>| {
        // SAFETY: The method has a single `#[output]` argument
        let output = unsafe { only_output(&method) };
        // Empty, although the return value of the previous method is in its memory
        assert_eq!(output.size, 0);
        assert_eq!(output.capacity, Address::SIZE);
        // SAFETY: Output buffer has the requested capacity
        unsafe {
            output.data_ptr.write_bytes(3, 3);
        }
        output.size = 3;

        Ok(())
    });
    let caller = call_next(caller, |method: PreparedMethod<'_>| {
        // SAFETY: The method has a single `#[input]` argument
        let input = unsafe { only_input(&method) };
        assert_eq!(input.size, 3);

        Ok(())
    });
    let result = caller.call_next_method_with(&CallWith::new(|_method: PreparedMethod<'_>| {
        unreachable!("The slot is rejected before the method is called")
    }));
    assert!(matches!(
        result,
        Err(TransactionPayloadCallError::Decoder(
            TransactionPayloadDecoderError::InvalidSlotOutputIndexSize {
                output_index: 0,
                size: 3
            }
        ))
    ));
}
