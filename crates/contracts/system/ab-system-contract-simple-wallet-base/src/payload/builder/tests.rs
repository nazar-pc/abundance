#![expect(unreachable_pub, reason = "Macro requirements and generated code")]

use crate::payload::builder::tests::ffi::get::DemoContractGetArgs;
use crate::payload::builder::tests::ffi::new::DemoContractNewArgs;
use crate::payload::builder::tests::ffi::set::DemoContractSetArgs;
use crate::payload::builder::tests::ffi::stored::DemoContractStoredArgs;
use crate::payload::builder::tests::ffi::value::DemoContractValueArgs;
use crate::payload::builder::{TransactionPayloadBuilder, TransactionPayloadBuilderError};
use crate::payload::decoder::{DecodedInput, DecodedOutput, OutputRef};
use crate::payload::test_utils::{CallWith, ExpectedMethod, alignment, assert_decoded};
use crate::payload::{
    FfiDataSizeCapacityRo, FfiDataSizeCapacityRw, TransactionMethodContext, TransactionOutput,
    TransactionOutputKind, TransactionOutputType, TransactionPayloadCaller,
};
use crate::{EXTERNAL_ARGS_BUFFER_SIZE, OUTPUT_BUFFER_OFFSETS_SIZE, OUTPUT_BUFFER_SIZE};
use ab_contracts_common::env::{MethodContext, PreparedMethod};
use ab_contracts_common::metadata::ContractMetadataKind;
use ab_contracts_common::metadata::decode::{ArgumentKind, MetadataDecodingError};
use ab_contracts_common::method::{ExternalArgs, MethodFingerprint};
use ab_contracts_macros::contract;
use ab_core_primitives::address::Address;
use ab_io_type::maybe_data::MaybeData;
use ab_io_type::metadata::IoTypeMetadataKind;
use ab_io_type::trivial_type::TrivialType;
use core::ffi::c_void;
use core::mem::MaybeUninit;
use core::ptr;
use core::ptr::NonNull;

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

    #[view]
    pub fn value(&self) -> u8 {
        self.value
    }

    #[view]
    pub fn stored(#[slot] value: &MaybeData<u8>) -> u8 {
        value.get().copied().unwrap_or_default()
    }
}

/// Capacity and type of the last output of the last method call added to `builder`, which end the
/// payload
fn last_output(builder: &TransactionPayloadBuilder) -> (u32, TransactionOutputType) {
    let (&output, before_output) = builder.payload.split_last().unwrap();
    let (_, &capacity) = before_output.split_last_chunk().unwrap();

    (
        u32::from_le_bytes(capacity),
        TransactionOutput::from_u8(output).unwrap().output_type(),
    )
}

// TODO: Test output indices and more complex types
#[test]
fn payload_encode_decode() {
    let expected_contract = Address::SYSTEM_SIMPLE_WALLET_BASE;
    let new_value = 42;

    let (payload, method_size) = {
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
        let method_size = builder.payload.len();
        (builder.into_aligned_bytes(), method_size)
    };

    assert_decoded(
        payload.as_u128_slice(),
        &[ExpectedMethod {
            contract: &expected_contract,
            fingerprint: &DemoContractSetArgs::FINGERPRINT,
            context: TransactionMethodContext::Wallet,
            slots: &[],
            inputs: &[DecodedInput::Value {
                bytes: &[new_value],
                alignment: alignment(1),
            }],
            outputs: &[],
        }],
    );

    let mut external_args_buffer = [ptr::null_mut(); EXTERNAL_ARGS_BUFFER_SIZE];
    let mut output_buffer = [MaybeUninit::uninit(); OUTPUT_BUFFER_SIZE];
    let mut output_buffer_details = [MaybeUninit::uninit(); OUTPUT_BUFFER_OFFSETS_SIZE];

    let caller = TransactionPayloadCaller::new(
        payload.as_u128_slice(),
        &mut external_args_buffer,
        &mut output_buffer,
        &mut output_buffer_details,
        |method_context| match method_context {
            TransactionMethodContext::Null => MethodContext::Reset,
            TransactionMethodContext::Wallet => MethodContext::Keep,
        },
    );

    let caller = caller
        .call_next_method_with(&CallWith::new(|method: PreparedMethod<'_>| {
            let PreparedMethod {
                contract,
                fingerprint,
                external_args,
                method_context,
                ..
            } = method;

            assert_eq!(contract, expected_contract);
            assert_eq!(fingerprint, DemoContractSetArgs::FINGERPRINT);
            assert_eq!(
                // SAFETY: method argument is a single `u8`
                unsafe { external_args.cast::<*const u8>().read().read() },
                new_value
            );
            assert_eq!(method_context, MethodContext::Keep);

            Ok(())
        }))
        .unwrap()
        .unwrap();

    // There is some padding, but it is correctly determined that there is no payload anymore
    assert!(payload.len() > method_size);
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

    assert_decoded(
        payload.as_u128_slice(),
        &[
            ExpectedMethod {
                contract: &expected_contract,
                fingerprint: &DemoContractGetArgs::FINGERPRINT,
                context: TransactionMethodContext::Null,
                slots: &[],
                inputs: &[],
                outputs: &[DecodedOutput {
                    output_index: 0,
                    kind: TransactionOutputKind::Output,
                    capacity: u8::SIZE,
                    size: 0,
                }],
            },
            ExpectedMethod {
                contract: &expected_contract,
                fingerprint: &DemoContractSetArgs::FINGERPRINT,
                context: TransactionMethodContext::Wallet,
                slots: &[],
                inputs: &[DecodedInput::Output(OutputRef {
                    output_index: 0,
                    kind: TransactionOutputKind::Output,
                    offset: 0,
                    size: 0,
                })],
                outputs: &[],
            },
        ],
    );

    let mut external_args_buffer = [ptr::null_mut(); EXTERNAL_ARGS_BUFFER_SIZE];
    let mut output_buffer = [MaybeUninit::uninit(); OUTPUT_BUFFER_SIZE];
    let mut output_buffer_details = [MaybeUninit::uninit(); OUTPUT_BUFFER_OFFSETS_SIZE];

    let caller = TransactionPayloadCaller::new(
        payload.as_u128_slice(),
        &mut external_args_buffer,
        &mut output_buffer,
        &mut output_buffer_details,
        |method_context| match method_context {
            TransactionMethodContext::Null => MethodContext::Reset,
            TransactionMethodContext::Wallet => MethodContext::Keep,
        },
    );

    let caller = caller
        .call_next_method_with(&CallWith::new(|method: PreparedMethod<'_>| {
            let PreparedMethod {
                contract,
                fingerprint,
                external_args,
                method_context,
                ..
            } = method;

            assert_eq!(contract, expected_contract);
            assert_eq!(fingerprint, DemoContractGetArgs::FINGERPRINT);
            assert_eq!(method_context, MethodContext::Reset);
            // Simulate the host writing the output of the method
            // SAFETY: The method has a single `#[output]` argument
            let output = unsafe { external_args.cast::<FfiDataSizeCapacityRw>().as_mut() };
            assert_eq!(output.size, 0);
            assert_eq!(output.capacity, u8::SIZE);
            // SAFETY: Output buffer has the requested capacity
            unsafe {
                output.data_ptr.write(output_value);
            }
            output.size = u8::SIZE;

            Ok(())
        }))
        .unwrap()
        .unwrap();

    let caller = caller
        .call_next_method_with(&CallWith::new(|method: PreparedMethod<'_>| {
            let PreparedMethod {
                contract,
                fingerprint,
                external_args,
                method_context,
                ..
            } = method;

            assert_eq!(contract, expected_contract);
            assert_eq!(fingerprint, DemoContractSetArgs::FINGERPRINT);
            assert_eq!(method_context, MethodContext::Keep);
            // SAFETY: The method has a single `#[input]` argument
            let input = unsafe { external_args.cast::<FfiDataSizeCapacityRo>().read() };
            assert_eq!(input.size, u8::SIZE);
            assert_eq!(input.capacity, u8::SIZE);
            // SAFETY: Input points to the output of the previous method
            assert_eq!(unsafe { input.data_ptr.read() }, output_value);

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
fn payload_encode_output_types() {
    let contract = Address::SYSTEM_SIMPLE_WALLET_BASE;

    // `#[output]` with the capacity of `MaybeData<u8>`
    {
        let mut value = 0;
        let mut size = 0;
        let mut builder = TransactionPayloadBuilder::new(1024);
        builder
            .with_method_call(
                &contract,
                &DemoContractGetArgs::new(&mut MaybeData::from_mut(&mut value, &mut size).unwrap()),
                TransactionMethodContext::Null,
                &[],
                &[],
            )
            .unwrap();

        assert_eq!(
            last_output(&builder),
            (u8::SIZE, TransactionOutputType::Output)
        );
        assert_decoded(
            builder.into_aligned_bytes().as_u128_slice(),
            &[ExpectedMethod {
                contract: &contract,
                fingerprint: &DemoContractGetArgs::FINGERPRINT,
                context: TransactionMethodContext::Null,
                slots: &[],
                inputs: &[],
                outputs: &[DecodedOutput {
                    output_index: 0,
                    kind: TransactionOutputKind::Output,
                    capacity: u8::SIZE,
                    size: 0,
                }],
            }],
        );
    }

    // Return value with the size of the return type as its capacity
    {
        let mut builder = TransactionPayloadBuilder::new(1024);
        builder
            .with_method_call(
                &contract,
                &DemoContractValueArgs::new(&mut MaybeUninit::uninit()),
                TransactionMethodContext::Null,
                &[],
                &[],
            )
            .unwrap();

        assert_eq!(
            last_output(&builder),
            (u8::SIZE, TransactionOutputType::ReturnValue)
        );
        assert_decoded(
            builder.into_aligned_bytes().as_u128_slice(),
            &[ExpectedMethod {
                contract: &contract,
                fingerprint: &DemoContractValueArgs::FINGERPRINT,
                context: TransactionMethodContext::Null,
                slots: &[],
                inputs: &[],
                outputs: &[DecodedOutput {
                    output_index: 0,
                    kind: TransactionOutputKind::ReturnValue,
                    capacity: u8::SIZE,
                    size: u8::SIZE,
                }],
            }],
        );
    }

    // The state returned by `#[init]` is not written to the caller, so it is an output without
    // capacity, which stays empty
    {
        let mut builder = TransactionPayloadBuilder::new(1024);
        builder
            .with_method_call(
                &contract,
                &DemoContractNewArgs::new(&1),
                TransactionMethodContext::Null,
                &[],
                &[],
            )
            .unwrap();

        assert_eq!(last_output(&builder), (0, TransactionOutputType::Output));
        assert_decoded(
            builder.into_aligned_bytes().as_u128_slice(),
            &[ExpectedMethod {
                contract: &contract,
                fingerprint: &DemoContractNewArgs::FINGERPRINT,
                context: TransactionMethodContext::Null,
                slots: &[],
                inputs: &[DecodedInput::Value {
                    bytes: &[1],
                    alignment: alignment(1),
                }],
                outputs: &[DecodedOutput {
                    output_index: 0,
                    kind: TransactionOutputKind::Output,
                    capacity: 0,
                    size: 0,
                }],
            }],
        );
    }
}

#[test]
fn payload_encode_decode_return_value() {
    let expected_contract = Address::SYSTEM_SIMPLE_WALLET_BASE;
    let return_value = 42;

    // The return value of the first method is used as the input of the second method
    let payload = {
        let mut builder = TransactionPayloadBuilder::new(1024);
        builder
            .with_method_call(
                &expected_contract,
                &DemoContractValueArgs::new(&mut MaybeUninit::uninit()),
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

    assert_decoded(
        payload.as_u128_slice(),
        &[
            ExpectedMethod {
                contract: &expected_contract,
                fingerprint: &DemoContractValueArgs::FINGERPRINT,
                context: TransactionMethodContext::Null,
                slots: &[],
                inputs: &[],
                outputs: &[DecodedOutput {
                    output_index: 0,
                    kind: TransactionOutputKind::ReturnValue,
                    capacity: u8::SIZE,
                    size: u8::SIZE,
                }],
            },
            ExpectedMethod {
                contract: &expected_contract,
                fingerprint: &DemoContractSetArgs::FINGERPRINT,
                context: TransactionMethodContext::Wallet,
                slots: &[],
                inputs: &[DecodedInput::Output(OutputRef {
                    output_index: 0,
                    kind: TransactionOutputKind::ReturnValue,
                    offset: 0,
                    size: u8::SIZE,
                })],
                outputs: &[],
            },
        ],
    );

    let mut external_args_buffer = [ptr::null_mut(); EXTERNAL_ARGS_BUFFER_SIZE];
    let mut output_buffer = [MaybeUninit::uninit(); OUTPUT_BUFFER_SIZE];
    let mut output_buffer_details = [MaybeUninit::uninit(); OUTPUT_BUFFER_OFFSETS_SIZE];

    let caller = TransactionPayloadCaller::new(
        payload.as_u128_slice(),
        &mut external_args_buffer,
        &mut output_buffer,
        &mut output_buffer_details,
        |method_context| match method_context {
            TransactionMethodContext::Null => MethodContext::Reset,
            TransactionMethodContext::Wallet => MethodContext::Keep,
        },
    );

    let caller = caller
        .call_next_method_with(&CallWith::new(|method: PreparedMethod<'_>| {
            let PreparedMethod {
                contract,
                fingerprint,
                external_args,
                method_context,
                ..
            } = method;

            assert_eq!(contract, expected_contract);
            assert_eq!(fingerprint, DemoContractValueArgs::FINGERPRINT);
            assert_eq!(method_context, MethodContext::Reset);
            // Simulate the host writing the return value of the method, which has no size in
            // `ExternalArgs`
            // SAFETY: The method has a single return value, whose pointer the caller follows with
            // its size and capacity
            let ok_result = unsafe { external_args.cast::<FfiDataSizeCapacityRw>().read() };
            assert_eq!(ok_result.size, u8::SIZE);
            assert_eq!(ok_result.capacity, u8::SIZE);
            // SAFETY: Output buffer has the requested capacity
            unsafe {
                ok_result.data_ptr.write(return_value);
            }

            Ok(())
        }))
        .unwrap()
        .unwrap();

    let caller = caller
        .call_next_method_with(&CallWith::new(|method: PreparedMethod<'_>| {
            let PreparedMethod {
                contract,
                fingerprint,
                external_args,
                method_context,
                ..
            } = method;

            assert_eq!(contract, expected_contract);
            assert_eq!(fingerprint, DemoContractSetArgs::FINGERPRINT);
            assert_eq!(method_context, MethodContext::Keep);
            // SAFETY: The method has a single `#[input]` argument
            let input = unsafe { external_args.cast::<FfiDataSizeCapacityRo>().read() };
            assert_eq!(input.size, u8::SIZE);
            assert_eq!(input.capacity, u8::SIZE);
            // SAFETY: Input points to the return value of the previous method
            assert_eq!(unsafe { input.data_ptr.read() }, return_value);

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
        let payload = builder.into_aligned_bytes();
        assert_eq!(payload.len(), payload_size, "Max size {max_size}");
        assert_decoded(
            payload.as_u128_slice(),
            &[ExpectedMethod {
                contract: &Address::SYSTEM_SIMPLE_WALLET_BASE,
                fingerprint: &DemoContractSetArgs::FINGERPRINT,
                context: TransactionMethodContext::Wallet,
                slots: &[],
                inputs: &[DecodedInput::Value {
                    bytes: &[42],
                    alignment: alignment(1),
                }],
                outputs: &[],
            }],
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

/// Add a call of a stateless method with `method_metadata` and the external args of a single `u8`
/// `#[input]` to a new builder
fn build_with_u8_input(
    method_metadata: &[u8],
) -> (
    TransactionPayloadBuilder,
    Result<(), TransactionPayloadBuilderError<'_>>,
) {
    let input_value = 42u8;
    // Outputs are not read from external args, so only the input is there
    let external_args = FfiDataSizeCapacityRo {
        data_ptr: NonNull::from_ref(&input_value),
        size: u8::SIZE,
        capacity: u8::SIZE,
    };
    let mut builder = TransactionPayloadBuilder::new(1024);
    // SAFETY: External args have the only input of the method
    let result = unsafe {
        builder.with_method_call_untyped(
            &Address::SYSTEM_SIMPLE_WALLET_BASE,
            NonNull::from_ref(&external_args).cast(),
            method_metadata,
            &DemoContractSetArgs::FINGERPRINT,
            TransactionMethodContext::Null,
            &[],
            &[],
        )
    };

    (builder, result)
}

#[test]
fn payload_encode_arguments_order() {
    // Metadata of a stateless method with an `#[input]` of `u8` and an `#[output]` of `u32`
    let method = [ContractMetadataKind::UpdateStateless as u8, 1, b'f', 2];
    let input_argument = [
        ContractMetadataKind::Input as u8,
        1,
        b'i',
        IoTypeMetadataKind::U8 as u8,
    ];
    let output_argument = [
        ContractMetadataKind::Output as u8,
        1,
        b'o',
        IoTypeMetadataKind::U32 as u8,
    ];

    // The order of `#[contract]`
    let ordered_metadata = [method, input_argument, output_argument].concat();
    let (builder, result) = build_with_u8_input(&ordered_metadata);
    result.unwrap();
    // The tag of the input follows the method header and the numbers of slots and inputs, and has
    // the alignment of `u8`
    let input_tag_offset = Address::SIZE as usize + MethodFingerprint::SIZE as usize + 3;
    assert_eq!(builder.payload[input_tag_offset], 0b1000_0000);
    assert_eq!(
        last_output(&builder),
        (u32::SIZE, TransactionOutputType::Output)
    );

    // The metadata decoder rejects an output before an input
    let output_first_metadata = [method, output_argument, input_argument].concat();
    let (_builder, result) = build_with_u8_input(&output_first_metadata);
    assert!(matches!(
        result,
        Err(TransactionPayloadBuilderError::MetadataDecodingError(
            MetadataDecodingError::ArgumentOutOfOrder {
                argument_kind: ArgumentKind::Input
            }
        ))
    ));
}

/// Add calls of `get()`, then of `set()` with its input and of `stored()` with its slot from the
/// output of `get()` to a new builder. `input_ptr` and `slot_ptr` are the pointers of that input
/// and slot in external args.
fn build_with_references(
    input_ptr: NonNull<u8>,
    slot_ptr: NonNull<Address>,
) -> TransactionPayloadBuilder {
    let contract = Address::SYSTEM_SIMPLE_WALLET_BASE;
    let mut unused_output = 0;
    let mut unused_output_size = 0;
    let mut builder = TransactionPayloadBuilder::new(1024);
    builder
        .with_method_call(
            &contract,
            &DemoContractGetArgs::new(
                &mut MaybeData::from_mut(&mut unused_output, &mut unused_output_size).unwrap(),
            ),
            TransactionMethodContext::Null,
            &[],
            &[],
        )
        .unwrap();

    // `ExternalArgs` of `set()` have the pointer, size and capacity of its input
    let set_args = FfiDataSizeCapacityRo {
        data_ptr: input_ptr,
        size: u8::SIZE,
        capacity: u8::SIZE,
    };
    // SAFETY: External args have the layout of `set()`, and its input refers to an earlier output
    unsafe {
        builder.with_method_call_untyped(
            &contract,
            NonNull::from_ref(&set_args).cast(),
            DemoContractSetArgs::METADATA,
            &DemoContractSetArgs::FINGERPRINT,
            TransactionMethodContext::Wallet,
            &[],
            &[Some(0)],
        )
    }
    .unwrap();

    // `ExternalArgs` of `stored()` have the pointers of its slot and of its return value, which
    // is an output and never read
    let stored_args = [slot_ptr.cast::<c_void>(), NonNull::dangling()];
    // SAFETY: External args have the layout of `stored()`, and its slot refers to an earlier
    // output
    unsafe {
        builder.with_method_call_untyped(
            &contract,
            NonNull::from_ref(&stored_args).cast(),
            DemoContractStoredArgs::METADATA,
            &DemoContractStoredArgs::FINGERPRINT,
            TransactionMethodContext::Null,
            &[Some(0)],
            &[],
        )
    }
    .unwrap();

    builder
}

#[test]
fn payload_untyped_references_not_dereferenced() {
    // Pointers of a slot and an input that refer to an earlier output may dangle, and Miri reports
    // a reference to such a pointer as undefined behavior
    let builder = build_with_references(NonNull::dangling(), NonNull::dangling());

    // Nothing is read through these pointers, so valid ones give the same payload
    let input_value = 42;
    let slot_address = Address::NULL;
    let expected_builder = build_with_references(
        NonNull::from_ref(&input_value),
        NonNull::from_ref(&slot_address),
    );
    assert_eq!(*builder.payload, *expected_builder.payload);
}
