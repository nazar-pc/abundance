#![expect(unreachable_pub, reason = "Macro requirements and generated code")]

use crate::payload::builder::tests::ffi::get::DemoContractGetArgs;
use crate::payload::builder::tests::ffi::set::DemoContractSetArgs;
use crate::payload::builder::{TransactionPayloadBuilder, TransactionPayloadBuilderError};
use crate::payload::{
    FfiDataSizeCapacityRo, FfiDataSizeCapacityRw, TransactionMethodContext,
    TransactionPayloadDecoder,
};
use crate::{EXTERNAL_ARGS_BUFFER_SIZE, OUTPUT_BUFFER_OFFSETS_SIZE, OUTPUT_BUFFER_SIZE};
use ab_contracts_common::env::{MethodContext, PreparedMethod};
use ab_contracts_common::method::ExternalArgs;
use ab_contracts_macros::contract;
use ab_core_primitives::address::Address;
use ab_io_type::maybe_data::MaybeData;
use ab_io_type::trivial_type::TrivialType;
use core::mem::MaybeUninit;
use core::ptr;

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct DemoContract {
    pub value: u8,
}

#[contract]
impl DemoContract {
    #[init]
    pub fn new(#[input] &init_value: &u8) -> Self {
        Self { value: init_value }
    }

    #[update]
    pub fn set(&mut self, #[input] &new_value: &u8) {
        self.value = new_value;
    }

    #[view]
    pub fn get(&self, #[output] value: &mut MaybeData<u8>) {
        value.replace(self.value);
    }
}

// TODO: Test output indices and more complex types
#[test]
fn payload_encode_decode() {
    let expected_contract = Address::SYSTEM_SIMPLE_WALLET_BASE;
    let new_value = 42;

    let payload = {
        let mut builder = TransactionPayloadBuilder::new(1024);
        builder
            .with_method_call(
                &expected_contract,
                &DemoContractSetArgs::new(&new_value),
                TransactionMethodContext::Wallet,
                &[],
                &[],
            )
            .unwrap();
        builder.into_aligned_bytes()
    };

    let mut external_args_buffer = [ptr::null_mut(); EXTERNAL_ARGS_BUFFER_SIZE];
    let mut output_buffer = [MaybeUninit::uninit(); OUTPUT_BUFFER_SIZE];
    let mut output_buffer_details = [MaybeUninit::uninit(); OUTPUT_BUFFER_OFFSETS_SIZE];

    let mut decoder = TransactionPayloadDecoder::new(
        payload.as_u128_slice(),
        &mut external_args_buffer,
        &mut output_buffer,
        &mut output_buffer_details,
        |method_context| match method_context {
            TransactionMethodContext::Null => MethodContext::Reset,
            TransactionMethodContext::Wallet => MethodContext::Keep,
        },
    );

    let PreparedMethod {
        contract,
        fingerprint,
        external_args,
        method_context,
        ..
    } = decoder.decode_next_method().unwrap().unwrap();

    assert_eq!(contract, expected_contract);
    assert_eq!(fingerprint, DemoContractSetArgs::FINGERPRINT);
    assert_eq!(
        // SAFETY: method argument is a single `u8`
        unsafe { external_args.cast::<*const u8>().read().read() },
        new_value
    );
    assert_eq!(method_context, MethodContext::Keep);

    // There is some padding, but it is correctly determined that there is no payload anymore
    assert!(!decoder.payload.is_empty());
    assert!(decoder.decode_next_method().unwrap().is_none());
}

#[test]
fn payload_encode_decode_output_index() {
    let expected_contract = Address::SYSTEM_SIMPLE_WALLET_BASE;
    let output_value = 42;

    // The output of the first method is used as the input of the second method
    let payload = {
        let mut unused_output = 0;
        let mut unused_output_size = 0;
        let mut builder = TransactionPayloadBuilder::new(1024);
        builder
            .with_method_call(
                &expected_contract,
                &DemoContractGetArgs::new(
                    &mut MaybeData::from_mut(&mut unused_output, &mut unused_output_size).unwrap(),
                ),
                TransactionMethodContext::Null,
                &[],
                &[],
            )
            .unwrap();
        builder
            .with_method_call(
                &expected_contract,
                &DemoContractSetArgs::new(&0),
                TransactionMethodContext::Wallet,
                &[],
                &[Some(0)],
            )
            .unwrap();
        builder.into_aligned_bytes()
    };

    let mut external_args_buffer = [ptr::null_mut(); EXTERNAL_ARGS_BUFFER_SIZE];
    let mut output_buffer = [MaybeUninit::uninit(); OUTPUT_BUFFER_SIZE];
    let mut output_buffer_details = [MaybeUninit::uninit(); OUTPUT_BUFFER_OFFSETS_SIZE];

    let mut decoder = TransactionPayloadDecoder::new(
        payload.as_u128_slice(),
        &mut external_args_buffer,
        &mut output_buffer,
        &mut output_buffer_details,
        |method_context| match method_context {
            TransactionMethodContext::Null => MethodContext::Reset,
            TransactionMethodContext::Wallet => MethodContext::Keep,
        },
    );

    let PreparedMethod {
        contract,
        fingerprint,
        external_args,
        method_context,
        ..
    } = decoder.decode_next_method().unwrap().unwrap();

    assert_eq!(contract, expected_contract);
    assert_eq!(fingerprint, DemoContractGetArgs::FINGERPRINT);
    assert_eq!(method_context, MethodContext::Reset);
    // Simulate the host writing the output of the method
    {
        // SAFETY: The method has a single `#[output]` argument
        let output = unsafe { external_args.cast::<FfiDataSizeCapacityRw>().as_mut() };
        assert_eq!(output.size, 0);
        assert_eq!(output.capacity, u8::SIZE);
        // SAFETY: Output buffer has the requested capacity
        unsafe {
            output.data_ptr.write(output_value);
        }
        output.size = u8::SIZE;
    }

    let PreparedMethod {
        contract,
        fingerprint,
        external_args,
        method_context,
        ..
    } = decoder.decode_next_method().unwrap().unwrap();

    assert_eq!(contract, expected_contract);
    assert_eq!(fingerprint, DemoContractSetArgs::FINGERPRINT);
    assert_eq!(method_context, MethodContext::Keep);
    {
        // SAFETY: The method has a single `#[input]` argument
        let input = unsafe { external_args.cast::<FfiDataSizeCapacityRo>().read() };
        assert_eq!(input.size, u8::SIZE);
        assert_eq!(input.capacity, u8::SIZE);
        // SAFETY: Input points to the output of the previous method
        assert_eq!(unsafe { input.data_ptr.read() }, output_value);
    }

    assert!(decoder.decode_next_method().unwrap().is_none());
}

/// Add a call of `DemoContract::set()` to `builder`
fn with_set_method_call(
    builder: &mut TransactionPayloadBuilder,
) -> Result<(), TransactionPayloadBuilderError<'static>> {
    builder.with_method_call(
        &Address::SYSTEM_SIMPLE_WALLET_BASE,
        &DemoContractSetArgs::new(&42),
        TransactionMethodContext::Wallet,
        &[],
        &[],
    )
}

#[test]
fn payload_max_size() {
    // Size of a payload with a single method call
    let payload_size = {
        let mut builder = TransactionPayloadBuilder::new(1024);
        with_set_method_call(&mut builder).unwrap();
        builder.into_aligned_bytes().len()
    };

    // The maximum size is rounded down to a multiple of `u128` size, which the payload fits into
    for max_size in [payload_size, payload_size + u128::SIZE - 1] {
        let mut builder = TransactionPayloadBuilder::new(max_size);
        with_set_method_call(&mut builder).unwrap();
        assert_eq!(
            builder.into_aligned_bytes().len(),
            payload_size,
            "Max size {max_size}"
        );
    }

    // A method call that doesn't fit is an error, the payload is not reallocated and nothing is
    // written past its maximum size
    for (max_size, num_method_calls) in [(payload_size - 1, 1), (payload_size, 2)] {
        let mut builder = TransactionPayloadBuilder::new(max_size);
        let capacity = builder.payload.capacity();
        let ptr = builder.payload.as_ptr();
        for _ in 1..num_method_calls {
            with_set_method_call(&mut builder).unwrap();
        }

        assert!(
            matches!(
                with_set_method_call(&mut builder),
                Err(TransactionPayloadBuilderError::PayloadTooLarge)
            ),
            "Max size {max_size}"
        );
        assert!(builder.payload.len() <= capacity, "Max size {max_size}");
        assert_eq!(builder.payload.capacity(), capacity, "Max size {max_size}");
        assert_eq!(builder.payload.as_ptr(), ptr, "Max size {max_size}");
    }
}
