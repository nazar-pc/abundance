#![expect(unreachable_pub, reason = "Macro requirements and generated code")]

use crate::payload::builder::tests::ffi::get::DemoContractGetArgs;
use crate::payload::builder::tests::ffi::get_value::DemoContractGetValueArgs;
use crate::payload::builder::tests::ffi::new::DemoContractNewArgs;
use crate::payload::builder::tests::ffi::owner::DemoContractOwnerArgs;
use crate::payload::builder::tests::ffi::set::DemoContractSetArgs;
use crate::payload::builder::tests::ffi::stored::DemoContractStoredArgs;
use crate::payload::builder::tests::ffi::value::DemoContractValueArgs;
use crate::payload::builder::tests::ffi::wide_value::DemoContractWideValueArgs;
use crate::payload::builder::{
    MethodCallReferences, TransactionPayloadBuilder, TransactionPayloadBuilderError,
};
use crate::payload::decoder::{DecodedInput, DecodedOutput, DecodedSlot, OutputRef};
use crate::payload::test_utils::{
    ExpectedMethod, alignment, assert_decoded, assert_no_more_methods, call_next, decode_all,
    new_caller, only_input, only_output,
};
use crate::payload::{
    FfiDataSizeCapacityRo, FfiDataSizeCapacityRw, TransactionMethodContext, TransactionOutputKind,
    TransactionPayloadDecoderError,
};
use crate::{EXTERNAL_ARGS_BUFFER_SIZE, MAX_OUTPUTS, OUTPUT_BUFFER_SIZE};
use ab_contracts_common::env::{MethodContext, PreparedMethod};
use ab_contracts_common::metadata::ContractMetadataKind;
use ab_contracts_common::metadata::decode::{ArgumentKind, MetadataDecodingError};
use ab_contracts_common::method::ExternalArgs;
use ab_contracts_macros::contract;
use ab_core_primitives::address::Address;
use ab_io_type::maybe_data::MaybeData;
use ab_io_type::metadata::IoTypeMetadataKind;
use ab_io_type::trivial_type::TrivialType;
use core::ffi::c_void;
use core::mem::MaybeUninit;
use core::ptr::NonNull;
use core::{array, ptr};

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
    pub fn wide_value(&self) -> u64 {
        u64::from(self.value)
    }

    #[view]
    pub fn stored(#[slot] value: &MaybeData<u8>) -> u8 {
        value.get().copied().unwrap_or_default()
    }

    #[view]
    pub fn get_value(&self, #[output] value: &mut MaybeData<u8>) -> u8 {
        value.replace(self.value);
        self.value
    }

    #[view]
    pub fn owner(&self) -> Address {
        Address::SYSTEM_SIMPLE_WALLET_BASE
    }
}

/// Description of a call of a method of `DemoContract` without arguments
fn expected_method<Args>(context: TransactionMethodContext) -> ExpectedMethod<'static>
where
    Args: ExternalArgs,
{
    ExpectedMethod {
        contract: &Address::SYSTEM_SIMPLE_WALLET_BASE,
        fingerprint: &Args::FINGERPRINT,
        context,
        slots: &[],
        inputs: &[],
        outputs: &[],
    }
}

/// Add a call of `DemoContract::new()` to `builder`, whose state is an `#[output]` that writes
/// `output_index`
fn with_new_method_call(
    builder: &mut TransactionPayloadBuilder,
    output_index: u8,
) -> Result<(), TransactionPayloadBuilderError<'static>> {
    builder.with_method_call(
        &Address::SYSTEM_SIMPLE_WALLET_BASE,
        &DemoContractNewArgs::new(&1),
        TransactionMethodContext::Null,
        MethodCallReferences {
            outputs: &[output_index],
            ..MethodCallReferences::default()
        },
    )
}

/// Add a call of `DemoContract::set()` to `builder`, with an input value or an input from
/// `input_output_index`
fn with_set_method_call(
    builder: &mut TransactionPayloadBuilder,
    input_output_index: Option<u8>,
) -> Result<(), TransactionPayloadBuilderError<'static>> {
    builder.with_method_call(
        &Address::SYSTEM_SIMPLE_WALLET_BASE,
        &DemoContractSetArgs::new(&42),
        TransactionMethodContext::Wallet,
        MethodCallReferences {
            inputs: &[input_output_index],
            ..MethodCallReferences::default()
        },
    )
}

/// Add a call of `DemoContract::get()` to `builder`, whose `#[output]` writes `output_index`
fn with_get_method_call(
    builder: &mut TransactionPayloadBuilder,
    output_index: u8,
) -> Result<(), TransactionPayloadBuilderError<'static>> {
    let mut value = 0;
    let mut size = 0;
    builder.with_method_call(
        &Address::SYSTEM_SIMPLE_WALLET_BASE,
        &DemoContractGetArgs::new(&mut MaybeData::from_mut(&mut value, &mut size).unwrap()),
        TransactionMethodContext::Null,
        MethodCallReferences {
            outputs: &[output_index],
            ..MethodCallReferences::default()
        },
    )
}

/// Add a call of `DemoContract::value()` to `builder`, whose return value writes `output_index`
fn with_value_method_call(
    builder: &mut TransactionPayloadBuilder,
    output_index: u8,
) -> Result<(), TransactionPayloadBuilderError<'static>> {
    builder.with_method_call(
        &Address::SYSTEM_SIMPLE_WALLET_BASE,
        &DemoContractValueArgs::new(&mut MaybeUninit::uninit()),
        TransactionMethodContext::Null,
        MethodCallReferences {
            outputs: &[output_index],
            ..MethodCallReferences::default()
        },
    )
}

/// Add a call of `DemoContract::wide_value()` to `builder`, whose return value writes
/// `output_index`
fn with_wide_value_method_call(
    builder: &mut TransactionPayloadBuilder,
    output_index: u8,
) -> Result<(), TransactionPayloadBuilderError<'static>> {
    builder.with_method_call(
        &Address::SYSTEM_SIMPLE_WALLET_BASE,
        &DemoContractWideValueArgs::new(&mut MaybeUninit::uninit()),
        TransactionMethodContext::Null,
        MethodCallReferences {
            outputs: &[output_index],
            ..MethodCallReferences::default()
        },
    )
}

/// Add a call of `DemoContract::stored()` to `builder`, with its slot from `slot_output_index` or
/// an explicit address, and its return value written into `output_index`
fn with_stored_method_call(
    builder: &mut TransactionPayloadBuilder,
    slot_output_index: Option<u8>,
    output_index: u8,
) -> Result<(), TransactionPayloadBuilderError<'static>> {
    builder.with_method_call(
        &Address::SYSTEM_SIMPLE_WALLET_BASE,
        &DemoContractStoredArgs::new(&Address::NULL, &mut MaybeUninit::uninit()),
        TransactionMethodContext::Null,
        MethodCallReferences {
            slots: &[slot_output_index],
            inputs: &[],
            outputs: &[output_index],
        },
    )
}

/// Add a call of `DemoContract::get_value()` to `builder`, whose `#[output]` and return value write
/// `output_indices`
fn with_get_value_method_call(
    builder: &mut TransactionPayloadBuilder,
    output_indices: &[u8],
) -> Result<(), TransactionPayloadBuilderError<'static>> {
    let mut value = 0;
    let mut size = 0;
    builder.with_method_call(
        &Address::SYSTEM_SIMPLE_WALLET_BASE,
        &DemoContractGetValueArgs::new(
            &mut MaybeData::from_mut(&mut value, &mut size).unwrap(),
            &mut MaybeUninit::uninit(),
        ),
        TransactionMethodContext::Null,
        MethodCallReferences {
            outputs: output_indices,
            ..MethodCallReferences::default()
        },
    )
}

/// Add a call of `DemoContract::owner()` to `builder`, whose return value, an address, writes
/// `output_index`
fn with_owner_method_call(
    builder: &mut TransactionPayloadBuilder,
    output_index: u8,
) -> Result<(), TransactionPayloadBuilderError<'static>> {
    builder.with_method_call(
        &Address::SYSTEM_SIMPLE_WALLET_BASE,
        &DemoContractOwnerArgs::new(&mut MaybeUninit::uninit()),
        TransactionMethodContext::Null,
        MethodCallReferences {
            outputs: &[output_index],
            ..MethodCallReferences::default()
        },
    )
}

/// Capacities in the header of the payload of `builder`, which declares `N` outputs
fn declared_capacities<const N: usize>(builder: &TransactionPayloadBuilder) -> [u32; N] {
    let payload = builder.payload.as_slice();
    assert_eq!(payload.first(), Some(&u8::try_from(N).unwrap()));

    // Capacities follow the number of declared outputs and 3 bytes of padding
    array::from_fn(|output_index| {
        let offset = size_of::<u32>() * (1 + output_index);
        u32::from_le_bytes(*payload[offset..].first_chunk().unwrap())
    })
}

/// Like the host: write `value` into the only `#[output]` or return value of `method`
fn write_output(method: &PreparedMethod<'_>, value: u8) {
    // SAFETY: The method has a single `#[output]` or return value, whose pointer the caller
    // follows with its size and capacity
    let output = unsafe { only_output(method) };
    assert_eq!(output.capacity, u8::SIZE);
    // SAFETY: Output buffer has the requested capacity
    unsafe {
        output.data_ptr.write(value);
    }
    output.size = u8::SIZE;
}

/// The value of the only `#[input]` of `method`, which has the size of `u8`
fn read_input(method: &PreparedMethod<'_>) -> u8 {
    // SAFETY: The method has a single `#[input]` argument
    let input = unsafe { only_input(method) };
    assert_eq!(input.size, u8::SIZE);
    assert_eq!(input.capacity, u8::SIZE);
    // SAFETY: The input points to an initialized `u8`
    unsafe { input.data_ptr.read() }
}

#[test]
fn payload_encode_decode() {
    let expected_contract = Address::SYSTEM_SIMPLE_WALLET_BASE;
    let new_value = 42;

    let (payload, unpadded_size) = {
        let mut builder = TransactionPayloadBuilder::new(1024, 0).unwrap();
        builder
            .with_method_call(
                &expected_contract,
                &DemoContractSetArgs::new(&new_value),
                TransactionMethodContext::Wallet,
                MethodCallReferences::default(),
            )
            .unwrap();
        let unpadded_size = builder.payload.len();
        (builder.into_aligned_bytes().unwrap(), unpadded_size)
    };

    assert_decoded(
        payload.as_u128_slice(),
        &[],
        &[ExpectedMethod {
            inputs: &[DecodedInput::Value {
                bytes: &[new_value],
                alignment: alignment(1),
            }],
            ..expected_method::<DemoContractSetArgs<'_>>(TransactionMethodContext::Wallet)
        }],
    );

    let mut external_args_buffer = [ptr::null_mut(); EXTERNAL_ARGS_BUFFER_SIZE];
    let mut output_buffer = [MaybeUninit::uninit(); OUTPUT_BUFFER_SIZE];
    let mut declared_outputs = [MaybeUninit::uninit(); MAX_OUTPUTS];
    let caller = new_caller(
        payload.as_u128_slice(),
        &mut external_args_buffer,
        &mut output_buffer,
        &mut declared_outputs,
    );

    let caller = call_next(caller, |method: PreparedMethod<'_>| {
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
    });

    // There is some padding, but it is correctly determined that there is no payload anymore
    assert!(payload.len() > unpadded_size);
    assert_no_more_methods(caller);
}

#[test]
fn payload_encode_decode_output_index() {
    let output_value = 42;

    // The output of the first method is used as the input of the second method
    let payload = {
        let mut builder = TransactionPayloadBuilder::new(1024, 1).unwrap();
        with_get_method_call(&mut builder, 0).unwrap();
        with_set_method_call(&mut builder, Some(0)).unwrap();
        builder.into_aligned_bytes().unwrap()
    };

    assert_decoded(
        payload.as_u128_slice(),
        &[u8::SIZE],
        &[
            ExpectedMethod {
                outputs: &[DecodedOutput {
                    output_index: 0,
                    kind: TransactionOutputKind::Output,
                    capacity: u8::SIZE,
                    size: 0,
                }],
                ..expected_method::<DemoContractGetArgs<'_>>(TransactionMethodContext::Null)
            },
            ExpectedMethod {
                inputs: &[DecodedInput::Output(OutputRef {
                    output_index: 0,
                    kind: TransactionOutputKind::Output,
                    offset: 0,
                    size: 0,
                })],
                ..expected_method::<DemoContractSetArgs<'_>>(TransactionMethodContext::Wallet)
            },
        ],
    );

    let mut external_args_buffer = [ptr::null_mut(); EXTERNAL_ARGS_BUFFER_SIZE];
    let mut output_buffer = [MaybeUninit::uninit(); OUTPUT_BUFFER_SIZE];
    let mut declared_outputs = [MaybeUninit::uninit(); MAX_OUTPUTS];
    let caller = new_caller(
        payload.as_u128_slice(),
        &mut external_args_buffer,
        &mut output_buffer,
        &mut declared_outputs,
    );

    let caller = call_next(caller, |method: PreparedMethod<'_>| {
        assert_eq!(method.fingerprint, DemoContractGetArgs::FINGERPRINT);
        assert_eq!(method.method_context, MethodContext::Reset);
        // SAFETY: The method has a single `#[output]` argument
        let output = unsafe { method.external_args.cast::<FfiDataSizeCapacityRw>().read() };
        assert_eq!(output.size, 0);
        write_output(&method, output_value);

        Ok(())
    });
    let caller = call_next(caller, |method: PreparedMethod<'_>| {
        assert_eq!(method.fingerprint, DemoContractSetArgs::FINGERPRINT);
        assert_eq!(method.method_context, MethodContext::Keep);
        assert_eq!(read_input(&method), output_value);

        Ok(())
    });
    assert_no_more_methods(caller);
}

#[test]
fn payload_encode_decode_return_value() {
    let return_value = 42;

    // The return value of the first method is used as the input of the second method
    let payload = {
        let mut builder = TransactionPayloadBuilder::new(1024, 1).unwrap();
        with_value_method_call(&mut builder, 0).unwrap();
        with_set_method_call(&mut builder, Some(0)).unwrap();
        builder.into_aligned_bytes().unwrap()
    };

    assert_decoded(
        payload.as_u128_slice(),
        &[u8::SIZE],
        &[
            ExpectedMethod {
                outputs: &[DecodedOutput {
                    output_index: 0,
                    kind: TransactionOutputKind::ReturnValue,
                    capacity: u8::SIZE,
                    size: u8::SIZE,
                }],
                ..expected_method::<DemoContractValueArgs<'_>>(TransactionMethodContext::Null)
            },
            ExpectedMethod {
                inputs: &[DecodedInput::Output(OutputRef {
                    output_index: 0,
                    kind: TransactionOutputKind::ReturnValue,
                    offset: 0,
                    size: u8::SIZE,
                })],
                ..expected_method::<DemoContractSetArgs<'_>>(TransactionMethodContext::Wallet)
            },
        ],
    );

    let mut external_args_buffer = [ptr::null_mut(); EXTERNAL_ARGS_BUFFER_SIZE];
    let mut output_buffer = [MaybeUninit::uninit(); OUTPUT_BUFFER_SIZE];
    let mut declared_outputs = [MaybeUninit::uninit(); MAX_OUTPUTS];
    let caller = new_caller(
        payload.as_u128_slice(),
        &mut external_args_buffer,
        &mut output_buffer,
        &mut declared_outputs,
    );

    let caller = call_next(caller, |method: PreparedMethod<'_>| {
        assert_eq!(method.fingerprint, DemoContractValueArgs::FINGERPRINT);
        // The return value has no size in `ExternalArgs`, the caller writes its declared size
        // SAFETY: The method has a single return value
        let ok_result = unsafe { method.external_args.cast::<FfiDataSizeCapacityRw>().read() };
        assert_eq!(ok_result.size, u8::SIZE);
        write_output(&method, return_value);

        Ok(())
    });
    let caller = call_next(caller, |method: PreparedMethod<'_>| {
        assert_eq!(method.fingerprint, DemoContractSetArgs::FINGERPRINT);
        assert_eq!(read_input(&method), return_value);

        Ok(())
    });
    assert_no_more_methods(caller);
}

#[test]
fn payload_encode_decode_reused_output() {
    // One declared output, which `get()` writes, `set()` reads, `value()` returns into, `set()`
    // reads again and `get()` writes once more
    let payload = {
        let mut builder = TransactionPayloadBuilder::new(1024, 1).unwrap();
        with_get_method_call(&mut builder, 0).unwrap();
        with_set_method_call(&mut builder, Some(0)).unwrap();
        with_value_method_call(&mut builder, 0).unwrap();
        with_set_method_call(&mut builder, Some(0)).unwrap();
        with_get_method_call(&mut builder, 0).unwrap();
        builder.into_aligned_bytes().unwrap()
    };

    let get = ExpectedMethod {
        outputs: &[DecodedOutput {
            output_index: 0,
            kind: TransactionOutputKind::Output,
            capacity: u8::SIZE,
            size: 0,
        }],
        ..expected_method::<DemoContractGetArgs<'_>>(TransactionMethodContext::Null)
    };
    let set = expected_method::<DemoContractSetArgs<'_>>(TransactionMethodContext::Wallet);
    assert_decoded(
        payload.as_u128_slice(),
        &[u8::SIZE],
        &[
            get,
            // The reference follows what was written last
            ExpectedMethod {
                inputs: &[DecodedInput::Output(OutputRef {
                    output_index: 0,
                    kind: TransactionOutputKind::Output,
                    offset: 0,
                    size: 0,
                })],
                ..set
            },
            ExpectedMethod {
                outputs: &[DecodedOutput {
                    output_index: 0,
                    kind: TransactionOutputKind::ReturnValue,
                    capacity: u8::SIZE,
                    size: u8::SIZE,
                }],
                ..expected_method::<DemoContractValueArgs<'_>>(TransactionMethodContext::Null)
            },
            ExpectedMethod {
                inputs: &[DecodedInput::Output(OutputRef {
                    output_index: 0,
                    kind: TransactionOutputKind::ReturnValue,
                    offset: 0,
                    size: u8::SIZE,
                })],
                ..set
            },
            get,
        ],
    );

    // Every method gets the same memory, and an `#[output]` gets it empty
    let mut external_args_buffer = [ptr::null_mut(); EXTERNAL_ARGS_BUFFER_SIZE];
    let mut output_buffer = [MaybeUninit::uninit(); OUTPUT_BUFFER_SIZE];
    let mut declared_outputs = [MaybeUninit::uninit(); MAX_OUTPUTS];
    let caller = new_caller(
        payload.as_u128_slice(),
        &mut external_args_buffer,
        &mut output_buffer,
        &mut declared_outputs,
    );
    let caller = call_next(caller, |method: PreparedMethod<'_>| {
        write_output(&method, 7);

        Ok(())
    });
    let caller = call_next(caller, |method: PreparedMethod<'_>| {
        assert_eq!(read_input(&method), 7);

        Ok(())
    });
    let caller = call_next(caller, |method: PreparedMethod<'_>| {
        write_output(&method, 9);

        Ok(())
    });
    let caller = call_next(caller, |method: PreparedMethod<'_>| {
        assert_eq!(read_input(&method), 9);

        Ok(())
    });
    let caller = call_next(caller, |method: PreparedMethod<'_>| {
        // SAFETY: The method has a single `#[output]` argument
        let output = unsafe { method.external_args.cast::<FfiDataSizeCapacityRw>().read() };
        assert_eq!(output.size, 0);

        Ok(())
    });
    assert_no_more_methods(caller);
}

#[test]
fn payload_encode_output_types() {
    let context = TransactionMethodContext::Null;

    // `#[output]` with the recommended capacity of `MaybeData<u8>`, whose tag ends the method
    let mut builder = TransactionPayloadBuilder::new(1024, 1).unwrap();
    // The number of declared outputs, 3 bytes of padding and the capacity, which starts at `0`
    assert_eq!(builder.payload.as_slice(), &[1, 0, 0, 0, 0, 0, 0, 0]);
    with_get_method_call(&mut builder, 0).unwrap();
    assert_eq!(builder.payload.as_slice()[..8], [1, 0, 0, 0, 1, 0, 0, 0]);
    assert_eq!(builder.payload.as_slice().last(), Some(&0b1000_0000));
    assert_decoded(
        builder.into_aligned_bytes().unwrap().as_u128_slice(),
        &[u8::SIZE],
        &[ExpectedMethod {
            outputs: &[DecodedOutput {
                output_index: 0,
                kind: TransactionOutputKind::Output,
                capacity: u8::SIZE,
                size: 0,
            }],
            ..expected_method::<DemoContractGetArgs<'_>>(context)
        }],
    );

    // Return value with the size of its type, which ends the method
    let mut builder = TransactionPayloadBuilder::new(1024, 1).unwrap();
    with_value_method_call(&mut builder, 0).unwrap();
    assert_eq!(declared_capacities(&builder), [u8::SIZE]);
    let (_, &return_value_size) = builder.payload.as_slice().split_last_chunk().unwrap();
    assert_eq!(u32::from_le_bytes(return_value_size), u8::SIZE);
    assert_decoded(
        builder.into_aligned_bytes().unwrap().as_u128_slice(),
        &[u8::SIZE],
        &[ExpectedMethod {
            outputs: &[DecodedOutput {
                output_index: 0,
                kind: TransactionOutputKind::ReturnValue,
                capacity: u8::SIZE,
                size: u8::SIZE,
            }],
            ..expected_method::<DemoContractValueArgs<'_>>(context)
        }],
    );

    // The state returned by `#[init]` is not written to the caller, so it is an `#[output]` without
    // capacity, which stays empty
    let mut builder = TransactionPayloadBuilder::new(1024, 1).unwrap();
    with_new_method_call(&mut builder, 0).unwrap();
    assert_decoded(
        builder.into_aligned_bytes().unwrap().as_u128_slice(),
        &[0],
        &[ExpectedMethod {
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
            ..expected_method::<DemoContractNewArgs<'_>>(context)
        }],
    );
}

#[test]
fn payload_output_capacity_of_all_uses() {
    // The capacity of a declared output is the largest that a method call writing it needs, in
    // whatever order they are added: the state of `#[init]` needs none, `get()` the recommended
    // capacity of `MaybeData<u8>` and `wide_value()` the size of `u64`
    let mut builder = TransactionPayloadBuilder::new(1024, 2).unwrap();
    with_new_method_call(&mut builder, 0).unwrap();
    with_new_method_call(&mut builder, 1).unwrap();
    assert_eq!(declared_capacities(&builder), [0, 0]);
    with_get_method_call(&mut builder, 0).unwrap();
    assert_eq!(declared_capacities(&builder), [u8::SIZE, 0]);
    with_wide_value_method_call(&mut builder, 0).unwrap();
    assert_eq!(declared_capacities(&builder), [u64::SIZE, 0]);
    with_get_method_call(&mut builder, 0).unwrap();
    with_new_method_call(&mut builder, 0).unwrap();
    assert_eq!(declared_capacities(&builder), [u64::SIZE, 0]);
    builder.into_aligned_bytes().unwrap();
}

#[test]
fn payload_unused_output() {
    // A declared output that no method call writes is an error
    let mut builder = TransactionPayloadBuilder::new(1024, 3).unwrap();
    with_get_method_call(&mut builder, 0).unwrap();
    with_value_method_call(&mut builder, 2).unwrap();
    assert!(matches!(
        builder.into_aligned_bytes(),
        Err(TransactionPayloadBuilderError::UnusedOutput(1))
    ));

    // The same without method calls
    let builder = TransactionPayloadBuilder::new(1024, 1).unwrap();
    assert!(matches!(
        builder.into_aligned_bytes(),
        Err(TransactionPayloadBuilderError::UnusedOutput(0))
    ));
}

#[test]
fn payload_too_many_outputs() {
    // A payload declares at most `MAX_OUTPUTS` outputs, and the decoder accepts one that declares
    // and writes all of them
    let max_outputs = u8::try_from(MAX_OUTPUTS).unwrap();
    let mut builder = TransactionPayloadBuilder::new(4096, max_outputs).unwrap();
    for output_index in 0..max_outputs {
        with_get_method_call(&mut builder, output_index).unwrap();
    }
    assert_eq!(declared_capacities(&builder), [u8::SIZE; MAX_OUTPUTS]);
    assert!(matches!(
        with_get_method_call(&mut builder, max_outputs),
        Err(TransactionPayloadBuilderError::InvalidOutputIndex(output_index))
            if output_index == max_outputs
    ));
    let payload = builder.into_aligned_bytes().unwrap();
    assert_eq!(decode_all(payload.as_u128_slice()), Ok(MAX_OUTPUTS));

    for num_outputs in [max_outputs + 1, u8::MAX] {
        assert!(
            matches!(
                TransactionPayloadBuilder::new(4096, num_outputs),
                Err(TransactionPayloadBuilderError::TooManyOutputs)
            ),
            "{num_outputs} outputs"
        );
    }
}

#[test]
fn payload_header_max_size() {
    // The header with 3 declared outputs fits into 16 bytes, but not with 4
    let mut builder = TransactionPayloadBuilder::new(u128::SIZE, 3).unwrap();
    assert_eq!(builder.payload.len(), u128::SIZE);
    assert_eq!(declared_capacities(&builder), [0; 3]);
    assert!(matches!(
        with_new_method_call(&mut builder, 0),
        Err(TransactionPayloadBuilderError::PayloadTooLarge)
    ));
    assert!(matches!(
        TransactionPayloadBuilder::new(u128::SIZE, 4),
        Err(TransactionPayloadBuilderError::PayloadTooLarge)
    ));

    // A maximum size below 16 bytes leaves no room for the header, even without declared outputs
    for num_outputs in [0, 1] {
        assert!(
            matches!(
                TransactionPayloadBuilder::new(u128::SIZE - 1, num_outputs),
                Err(TransactionPayloadBuilderError::PayloadTooLarge)
            ),
            "{num_outputs} outputs"
        );
    }
}

#[test]
fn payload_method_call_references_mismatch() {
    let mut builder = TransactionPayloadBuilder::new(1024, 2).unwrap();
    let mut value = 0;
    let mut size = 0;
    let mut maybe_data = MaybeData::from_mut(&mut value, &mut size).unwrap();

    // `get()` has one output
    for outputs in [&[][..], &[0, 1]] {
        assert!(
            matches!(
                builder.with_method_call(
                    &Address::SYSTEM_SIMPLE_WALLET_BASE,
                    &DemoContractGetArgs::new(&mut maybe_data),
                    TransactionMethodContext::Null,
                    MethodCallReferences {
                        outputs,
                        ..MethodCallReferences::default()
                    },
                ),
                Err(TransactionPayloadBuilderError::NumberOfOutputsMismatch {
                    expected: 1,
                    actual
                }) if actual == outputs.len()
            ),
            "Outputs {outputs:?}"
        );
    }

    // `set()` has one input and no slots
    assert!(matches!(
        builder.with_method_call(
            &Address::SYSTEM_SIMPLE_WALLET_BASE,
            &DemoContractSetArgs::new(&1),
            TransactionMethodContext::Null,
            MethodCallReferences {
                slots: &[None],
                ..MethodCallReferences::default()
            },
        ),
        Err(TransactionPayloadBuilderError::TooManySlotReferences {
            num_slots: 0,
            num_references: 1
        })
    ));
    assert!(matches!(
        builder.with_method_call(
            &Address::SYSTEM_SIMPLE_WALLET_BASE,
            &DemoContractSetArgs::new(&1),
            TransactionMethodContext::Null,
            MethodCallReferences {
                inputs: &[Some(0), None],
                ..MethodCallReferences::default()
            },
        ),
        Err(TransactionPayloadBuilderError::TooManyInputReferences {
            num_inputs: 1,
            num_references: 2
        })
    ));

    // Nothing was added or changed, the payload has only its header
    assert_eq!(builder.payload.len(), 12);
    assert_eq!(declared_capacities(&builder), [0, 0]);
}

#[test]
fn payload_undeclared_output() {
    let mut builder = TransactionPayloadBuilder::new(1024, 1).unwrap();
    let mut value = 0;
    let mut size = 0;
    let mut maybe_data = MaybeData::from_mut(&mut value, &mut size).unwrap();

    // Output index `1` isn't declared, in a slot, an input and an output, which the decoder
    // rejects with the same error
    assert!(matches!(
        builder.with_method_call(
            &Address::SYSTEM_SIMPLE_WALLET_BASE,
            &DemoContractStoredArgs::new(&Address::NULL, &mut MaybeUninit::uninit()),
            TransactionMethodContext::Null,
            MethodCallReferences {
                slots: &[Some(1)],
                inputs: &[],
                outputs: &[0],
            },
        ),
        Err(TransactionPayloadBuilderError::PayloadRule(
            TransactionPayloadDecoderError::OutputIndexNotFound(1)
        ))
    ));
    assert!(matches!(
        with_set_method_call(&mut builder, Some(1)),
        Err(TransactionPayloadBuilderError::PayloadRule(
            TransactionPayloadDecoderError::OutputIndexNotFound(1)
        ))
    ));
    assert!(matches!(
        builder.with_method_call(
            &Address::SYSTEM_SIMPLE_WALLET_BASE,
            &DemoContractGetArgs::new(&mut maybe_data),
            TransactionMethodContext::Null,
            MethodCallReferences {
                outputs: &[1],
                ..MethodCallReferences::default()
            },
        ),
        Err(TransactionPayloadBuilderError::PayloadRule(
            TransactionPayloadDecoderError::OutputIndexNotFound(1)
        ))
    ));
    // The first call wrote nothing, although its output index `0` is valid
    assert_eq!(declared_capacities(&builder), [0]);

    // A method call that breaks several rules gets the error of the rule that the decoder checks
    // first: its slot refers to declared output `0`, which no earlier method call wrote, and it
    // writes output index `1`, which isn't declared
    assert!(matches!(
        with_stored_method_call(&mut builder, Some(0), 1),
        Err(TransactionPayloadBuilderError::PayloadRule(
            TransactionPayloadDecoderError::OutputIndexNotFound(0)
        ))
    ));

    // The same after a method call was added, whose bytes follow the declared outputs
    with_set_method_call(&mut builder, None).unwrap();
    assert!(matches!(
        with_get_method_call(&mut builder, 1),
        Err(TransactionPayloadBuilderError::PayloadRule(
            TransactionPayloadDecoderError::OutputIndexNotFound(1)
        ))
    ));
}

#[test]
fn payload_invalid_output_index() {
    // Tags can't hold output indices from `MAX_OUTPUTS` on, in a slot, an input and an output
    let mut builder = TransactionPayloadBuilder::new(1024, 1).unwrap();
    with_get_method_call(&mut builder, 0).unwrap();
    for output_index in [u8::try_from(MAX_OUTPUTS).unwrap(), u8::MAX] {
        assert!(
            matches!(
                with_stored_method_call(&mut builder, Some(output_index), 0),
                Err(TransactionPayloadBuilderError::InvalidOutputIndex(invalid_output_index))
                    if invalid_output_index == output_index
            ),
            "Output index {output_index}"
        );
        assert!(
            matches!(
                with_set_method_call(&mut builder, Some(output_index)),
                Err(TransactionPayloadBuilderError::InvalidOutputIndex(invalid_output_index))
                    if invalid_output_index == output_index
            ),
            "Output index {output_index}"
        );
        assert!(
            matches!(
                with_get_method_call(&mut builder, output_index),
                Err(TransactionPayloadBuilderError::InvalidOutputIndex(invalid_output_index))
                    if invalid_output_index == output_index
            ),
            "Output index {output_index}"
        );
    }
    builder.into_aligned_bytes().unwrap();
}

#[test]
fn payload_reference_to_unwritten_output() {
    // A slot and an input that refer to a declared output that no earlier method call wrote
    let mut builder = TransactionPayloadBuilder::new(1024, 2).unwrap();
    assert!(matches!(
        with_set_method_call(&mut builder, Some(0)),
        Err(TransactionPayloadBuilderError::PayloadRule(
            TransactionPayloadDecoderError::OutputIndexNotFound(0)
        ))
    ));
    assert!(matches!(
        with_stored_method_call(&mut builder, Some(0), 1),
        Err(TransactionPayloadBuilderError::PayloadRule(
            TransactionPayloadDecoderError::OutputIndexNotFound(0)
        ))
    ));
    // Nothing was added or changed, the payload has only its header
    assert_eq!(builder.payload.len(), 12);
    assert_eq!(declared_capacities(&builder), [0, 0]);

    // After a method call wrote it, both are accepted
    with_get_method_call(&mut builder, 0).unwrap();
    with_set_method_call(&mut builder, Some(0)).unwrap();
    with_stored_method_call(&mut builder, Some(0), 1).unwrap();
    builder.into_aligned_bytes().unwrap();
}

#[test]
fn payload_reference_to_own_output() {
    let mut builder = TransactionPayloadBuilder::new(1024, 2).unwrap();
    with_get_method_call(&mut builder, 0).unwrap();
    with_get_method_call(&mut builder, 1).unwrap();

    // A slot that refers to the declared output that the return value of the same method call
    // writes
    assert!(matches!(
        with_stored_method_call(&mut builder, Some(0), 0),
        Err(TransactionPayloadBuilderError::PayloadRule(
            TransactionPayloadDecoderError::OutputReferencedBySameMethod(0)
        ))
    ));
    // An input that refers to the declared output that the state of the same `#[init]` writes
    assert!(matches!(
        builder.with_method_call(
            &Address::SYSTEM_SIMPLE_WALLET_BASE,
            &DemoContractNewArgs::new(&1),
            TransactionMethodContext::Null,
            MethodCallReferences {
                inputs: &[Some(1)],
                outputs: &[1],
                ..MethodCallReferences::default()
            },
        ),
        Err(TransactionPayloadBuilderError::PayloadRule(
            TransactionPayloadDecoderError::OutputReferencedBySameMethod(1)
        ))
    ));
    let header_and_two_calls = builder.payload.len();

    // Referring to the other declared output is accepted
    with_stored_method_call(&mut builder, Some(1), 0).unwrap();
    assert!(builder.payload.len() > header_and_two_calls);
    builder.into_aligned_bytes().unwrap();
}

#[test]
fn payload_output_written_twice() {
    // `get_value()` writes the same declared output with its `#[output]` and its return value
    let mut builder = TransactionPayloadBuilder::new(1024, 2).unwrap();
    assert!(matches!(
        with_get_value_method_call(&mut builder, &[1, 1]),
        Err(TransactionPayloadBuilderError::PayloadRule(
            TransactionPayloadDecoderError::OutputUsedTwice(1)
        ))
    ));
    assert_eq!(declared_capacities(&builder), [0, 0]);

    // Different declared outputs are accepted
    with_get_value_method_call(&mut builder, &[1, 0]).unwrap();
    assert_eq!(declared_capacities(&builder), [u8::SIZE, u8::SIZE]);
    builder.into_aligned_bytes().unwrap();
}

#[test]
fn payload_slot_from_return_value_size() {
    // A slot can refer to a return value only if it has the size of an address
    let mut builder = TransactionPayloadBuilder::new(1024, 2).unwrap();
    with_value_method_call(&mut builder, 0).unwrap();
    assert!(matches!(
        with_stored_method_call(&mut builder, Some(0), 1),
        Err(TransactionPayloadBuilderError::PayloadRule(
            TransactionPayloadDecoderError::InvalidSlotOutputIndexSize {
                output_index: 0,
                size: 1
            }
        ))
    ));

    // An address returned into the same declared output is accepted, and so is an `#[output]`,
    // whose size is only known when its method is called
    with_owner_method_call(&mut builder, 0).unwrap();
    with_stored_method_call(&mut builder, Some(0), 1).unwrap();
    with_get_method_call(&mut builder, 0).unwrap();
    with_stored_method_call(&mut builder, Some(0), 1).unwrap();
    builder.into_aligned_bytes().unwrap();
}

/// Pseudo-random numbers with a fixed sequence for each seed (xorshift64*)
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A number below `n`
    fn below(&mut self, n: u8) -> u8 {
        u8::try_from(self.next() % u64::from(n)).unwrap()
    }
}

/// An output index for a builder that declares `num_outputs` outputs, which is not declared now
/// and then
fn random_output_index(rng: &mut Rng, num_outputs: u8) -> u8 {
    rng.below(num_outputs + 1)
}

/// A reference to an output index, see [`random_output_index()`], or none
fn random_reference(rng: &mut Rng, num_outputs: u8) -> Option<u8> {
    if rng.below(3) == 0 {
        None
    } else {
        Some(random_output_index(rng, num_outputs))
    }
}

/// Add a call of a random method of `DemoContract` with random references to `builder`, which
/// declares `num_outputs` outputs
fn with_random_method_call(
    builder: &mut TransactionPayloadBuilder,
    rng: &mut Rng,
    num_outputs: u8,
) -> Result<(), TransactionPayloadBuilderError<'static>> {
    let reference = random_reference(rng, num_outputs);
    let output_index = random_output_index(rng, num_outputs);
    match rng.below(7) {
        0 => builder.with_method_call(
            &Address::SYSTEM_SIMPLE_WALLET_BASE,
            &DemoContractNewArgs::new(&1),
            TransactionMethodContext::Null,
            MethodCallReferences {
                inputs: &[reference],
                outputs: &[output_index],
                ..MethodCallReferences::default()
            },
        ),
        1 => with_set_method_call(builder, reference),
        2 => with_get_method_call(builder, output_index),
        3 => with_value_method_call(builder, output_index),
        4 => with_owner_method_call(builder, output_index),
        5 => with_stored_method_call(builder, reference, output_index),
        _ => with_get_value_method_call(
            builder,
            &[output_index, random_output_index(rng, num_outputs)],
        ),
    }
}

#[test]
fn payload_accepted_by_builder_decodes() {
    // Random method calls with random references, some of which break rules of the payload
    // format. The builder must reject those, so that every payload it builds is accepted by the
    // decoder with all the method calls that the builder accepted. Some payloads are too small
    // for all method calls, and a method call that doesn't fit must not change the builder.
    let mut rng = Rng(1);
    let mut num_payloads = 0;
    let mut num_rejected = 0;
    let mut num_too_large = 0;
    for _ in 0..1000 {
        let num_outputs = rng.below(4);
        let max_size = if rng.below(4) == 0 {
            u128::SIZE * (1 + u32::from(rng.below(16)))
        } else {
            4096
        };
        let mut builder = TransactionPayloadBuilder::new(max_size, num_outputs).unwrap();
        let mut num_methods = 0;
        for _ in 0..rng.below(10) {
            match with_random_method_call(&mut builder, &mut rng, num_outputs) {
                Ok(()) => num_methods += 1,
                Err(TransactionPayloadBuilderError::PayloadRule(_)) => num_rejected += 1,
                Err(TransactionPayloadBuilderError::PayloadTooLarge) => num_too_large += 1,
                Err(error) => panic!("Unexpected error: {error}"),
            }
        }
        // A declared output that no method call writes is the builder's own error
        let Ok(payload) = builder.into_aligned_bytes() else {
            continue;
        };

        assert_eq!(decode_all(payload.as_u128_slice()), Ok(num_methods));
        num_payloads += 1;
    }

    // All of them happen often, so that the test checks something
    assert!(num_payloads > 100, "{num_payloads} payloads");
    assert!(num_rejected > 100, "{num_rejected} rejected method calls");
    assert!(
        num_too_large > 100,
        "{num_too_large} method calls that didn't fit"
    );
}

#[test]
fn payload_untyped_references_not_dereferenced() {
    let mut builder = TransactionPayloadBuilder::new(1024, 2).unwrap();
    let output_index = 0;
    let return_value_index = 1;
    with_get_method_call(&mut builder, output_index).unwrap();

    // Pointers of a slot and an input that refer to a declared output can be dangling, Miri reports
    // a reference to them as undefined behavior. `ExternalArgs` of `set()` have the pointer, size
    // and capacity of its input.
    let set_args = FfiDataSizeCapacityRo {
        data_ptr: NonNull::dangling(),
        size: u8::SIZE,
        capacity: u8::SIZE,
    };
    // SAFETY: The layout corresponds to the method, and the input refers to a declared output
    unsafe {
        builder.with_method_call_untyped(
            &Address::SYSTEM_SIMPLE_WALLET_BASE,
            NonNull::from_ref(&set_args).cast::<*const c_void>(),
            DemoContractSetArgs::METADATA,
            &DemoContractSetArgs::FINGERPRINT,
            TransactionMethodContext::Wallet,
            MethodCallReferences {
                inputs: &[Some(output_index)],
                ..MethodCallReferences::default()
            },
        )
    }
    .unwrap();
    // `ExternalArgs` of `stored()` have the pointer of its slot and of its return value
    let stored_args = [
        NonNull::<Address>::dangling().cast::<c_void>(),
        NonNull::dangling(),
    ];
    // SAFETY: The layout corresponds to the method, and the slot refers to a declared output
    unsafe {
        builder.with_method_call_untyped(
            &Address::SYSTEM_SIMPLE_WALLET_BASE,
            NonNull::from_ref(&stored_args).cast::<*const c_void>(),
            DemoContractStoredArgs::METADATA,
            &DemoContractStoredArgs::FINGERPRINT,
            TransactionMethodContext::Null,
            MethodCallReferences {
                slots: &[Some(output_index)],
                inputs: &[],
                outputs: &[return_value_index],
            },
        )
    }
    .unwrap();

    let output_ref = OutputRef {
        output_index,
        kind: TransactionOutputKind::Output,
        offset: 0,
        size: 0,
    };
    assert_decoded(
        builder.into_aligned_bytes().unwrap().as_u128_slice(),
        &[u8::SIZE, u8::SIZE],
        &[
            ExpectedMethod {
                outputs: &[DecodedOutput {
                    output_index,
                    kind: TransactionOutputKind::Output,
                    capacity: u8::SIZE,
                    size: 0,
                }],
                ..expected_method::<DemoContractGetArgs<'_>>(TransactionMethodContext::Null)
            },
            ExpectedMethod {
                inputs: &[DecodedInput::Output(output_ref)],
                ..expected_method::<DemoContractSetArgs<'_>>(TransactionMethodContext::Wallet)
            },
            ExpectedMethod {
                slots: &[DecodedSlot::Output(output_ref)],
                outputs: &[DecodedOutput {
                    output_index: return_value_index,
                    kind: TransactionOutputKind::ReturnValue,
                    capacity: u8::SIZE,
                    size: u8::SIZE,
                }],
                ..expected_method::<DemoContractStoredArgs<'_>>(TransactionMethodContext::Null)
            },
        ],
    );
}

#[test]
fn payload_empty() {
    // A builder without declared outputs and method calls makes a payload with only the header,
    // which declares no outputs, padded to 16 bytes
    let builder = TransactionPayloadBuilder::new(1024, 0).unwrap();
    assert_eq!(builder.payload.as_slice(), &[0]);
    let payload = builder.into_aligned_bytes().unwrap();
    assert_eq!(payload.as_slice(), &[0; u128::SIZE as usize]);
    assert_decoded(payload.as_u128_slice(), &[], &[]);

    // With room for the header only, no method call can be added
    let mut builder = TransactionPayloadBuilder::new(u128::SIZE, 0).unwrap();
    assert!(matches!(
        with_set_method_call(&mut builder, None),
        Err(TransactionPayloadBuilderError::PayloadTooLarge)
    ));
}

#[test]
fn payload_max_size() {
    // Size of a payload with a single method call
    let payload_size = {
        let mut builder = TransactionPayloadBuilder::new(1024, 0).unwrap();
        with_set_method_call(&mut builder, None).unwrap();
        builder.into_aligned_bytes().unwrap().len()
    };

    // The maximum size is rounded down to a multiple of `u128` size, which the payload fits into
    for max_size in [payload_size, payload_size + u128::SIZE - 1] {
        let mut builder = TransactionPayloadBuilder::new(max_size, 0).unwrap();
        with_set_method_call(&mut builder, None).unwrap();
        let payload = builder.into_aligned_bytes().unwrap();
        assert_eq!(payload.len(), payload_size, "Max size {max_size}");
        assert_decoded(
            payload.as_u128_slice(),
            &[],
            &[ExpectedMethod {
                inputs: &[DecodedInput::Value {
                    bytes: &[42],
                    alignment: alignment(1),
                }],
                ..expected_method::<DemoContractSetArgs<'_>>(TransactionMethodContext::Wallet)
            }],
        );
    }

    // A method call that doesn't fit is an error, the payload is not reallocated and nothing is
    // written past its maximum size
    for (max_size, num_method_calls) in [(payload_size - 1, 1), (payload_size, 2)] {
        let mut builder = TransactionPayloadBuilder::new(max_size, 0).unwrap();
        let capacity = builder.payload.capacity();
        let ptr = builder.payload.as_ptr();
        for _ in 1..num_method_calls {
            with_set_method_call(&mut builder, None).unwrap();
        }

        assert!(
            matches!(
                with_set_method_call(&mut builder, None),
                Err(TransactionPayloadBuilderError::PayloadTooLarge)
            ),
            "Max size {max_size}"
        );
        assert!(builder.payload.len() <= capacity, "Max size {max_size}");
        assert_eq!(builder.payload.capacity(), capacity, "Max size {max_size}");
        assert_eq!(builder.payload.as_ptr(), ptr, "Max size {max_size}");
    }
}

#[test]
fn payload_too_large_leaves_builder_unchanged() {
    // Size of the header and a single method call
    let payload_size = {
        let mut builder = TransactionPayloadBuilder::new(1024, 1).unwrap();
        with_get_method_call(&mut builder, 0).unwrap();
        builder.payload.len()
    };

    // Room for one method call, but not for a second one, which would raise the capacity of the
    // declared output if it was added
    let mut builder =
        TransactionPayloadBuilder::new(payload_size.next_multiple_of(u128::SIZE), 1).unwrap();
    with_get_method_call(&mut builder, 0).unwrap();
    let payload = builder.payload.clone();
    assert!(matches!(
        with_wide_value_method_call(&mut builder, 0),
        Err(TransactionPayloadBuilderError::PayloadTooLarge)
    ));
    assert_eq!(builder.payload.as_slice(), payload.as_slice());
    assert_eq!(declared_capacities(&builder), [u8::SIZE]);

    // The payload has the first method call only
    let payload = builder.into_aligned_bytes().unwrap();
    assert_eq!(decode_all(payload.as_u128_slice()), Ok(1));

    // A method call that doesn't fit right after the header leaves the declared output unwritten
    let mut builder = TransactionPayloadBuilder::new(payload_size - 1, 1).unwrap();
    assert!(matches!(
        with_get_method_call(&mut builder, 0),
        Err(TransactionPayloadBuilderError::PayloadTooLarge)
    ));
    assert_eq!(builder.payload.len(), 8);
    assert_eq!(declared_capacities(&builder), [0]);
    assert!(matches!(
        builder.into_aligned_bytes(),
        Err(TransactionPayloadBuilderError::UnusedOutput(0))
    ));
}

/// Add a call of a stateless method with `method_metadata` and the external args of a single `u32`
/// `#[input]` to a new builder, whose only declared output the method writes
fn build_with_u32_input(
    method_metadata: &[u8],
) -> (
    TransactionPayloadBuilder,
    Result<(), TransactionPayloadBuilderError<'_>>,
) {
    let input_value = 42u32;
    // Outputs are not read from external args, so only the input is there
    let external_args = FfiDataSizeCapacityRo {
        data_ptr: NonNull::from_ref(&input_value).cast::<u8>(),
        size: u32::SIZE,
        capacity: u32::SIZE,
    };
    let mut builder = TransactionPayloadBuilder::new(1024, 1).unwrap();
    // SAFETY: External args have the only input of the method
    let result = unsafe {
        builder.with_method_call_untyped(
            &Address::SYSTEM_SIMPLE_WALLET_BASE,
            NonNull::from_ref(&external_args).cast(),
            method_metadata,
            &DemoContractSetArgs::FINGERPRINT,
            TransactionMethodContext::Null,
            MethodCallReferences {
                outputs: &[0],
                ..MethodCallReferences::default()
            },
        )
    };

    (builder, result)
}

#[test]
fn payload_encode_arguments_order() {
    // Metadata of a stateless method with an `#[input]` of `u32` and an `#[output]` of `u64`
    let method = [ContractMetadataKind::UpdateStateless as u8, 1, b'f', 2];
    let input_argument = [
        ContractMetadataKind::Input as u8,
        1,
        b'i',
        IoTypeMetadataKind::U32 as u8,
    ];
    let output_argument = [
        ContractMetadataKind::Output as u8,
        1,
        b'o',
        IoTypeMetadataKind::U64 as u8,
    ];

    // The order of `#[contract]`: the input value has the alignment of its type, and the output
    // gets the recommended capacity of its type. The types differ, so the details of the input
    // and the output can't be mixed up.
    let ordered_metadata = [method, input_argument, output_argument].concat();
    let (builder, result) = build_with_u32_input(&ordered_metadata);
    result.unwrap();
    assert_decoded(
        builder.into_aligned_bytes().unwrap().as_u128_slice(),
        &[u64::SIZE],
        &[ExpectedMethod {
            inputs: &[DecodedInput::Value {
                bytes: &42u32.to_le_bytes(),
                alignment: alignment(4),
            }],
            outputs: &[DecodedOutput {
                output_index: 0,
                kind: TransactionOutputKind::Output,
                capacity: u64::SIZE,
                size: 0,
            }],
            ..expected_method::<DemoContractSetArgs<'_>>(TransactionMethodContext::Null)
        }],
    );

    // The metadata decoder rejects an output before an input
    let output_first_metadata = [method, output_argument, input_argument].concat();
    let (_builder, result) = build_with_u32_input(&output_first_metadata);
    assert!(matches!(
        result,
        Err(TransactionPayloadBuilderError::MetadataDecodingError(
            MetadataDecodingError::ArgumentOutOfOrder {
                argument_kind: ArgumentKind::Input
            }
        ))
    ));
}
