#![expect(unreachable_pub, reason = "Macro requirements and generated code")]

use crate::payload::builder::TransactionPayloadBuilder;
use crate::payload::builder::tests::ffi::set::DemoContractSetArgs;
use crate::payload::{TransactionMethodContext, TransactionPayloadDecoder};
use crate::{EXTERNAL_ARGS_BUFFER_SIZE, OUTPUT_BUFFER_OFFSETS_SIZE, OUTPUT_BUFFER_SIZE};
use ab_contracts_common::env::{MethodContext, PreparedMethod};
use ab_contracts_common::method::ExternalArgs;
use ab_contracts_macros::contract;
use ab_core_primitives::address::Address;
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
    pub fn get(&self, #[output] value: &mut u8) {
        *value = self.value;
    }
}

// TODO: Test output indices and more complex types
#[test]
fn payload_encode_decode() {
    let expected_contract = Address::SYSTEM_SIMPLE_WALLET_BASE;
    let new_value = 42;

    let payload = {
        let mut builder = TransactionPayloadBuilder::default();
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

    // Untrusted
    {
        let mut decoder = TransactionPayloadDecoder::new(
            &payload,
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
            phantom: _,
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

    // Trusted
    {
        let mut decoder = TransactionPayloadDecoder::new(
            &payload,
            &mut external_args_buffer,
            &mut output_buffer,
            &mut output_buffer_details,
            |method_context| match method_context {
                TransactionMethodContext::Null => MethodContext::Reset,
                TransactionMethodContext::Wallet => MethodContext::Keep,
            },
        );

        // SAFETY: Trusted input
        let PreparedMethod {
            contract,
            fingerprint,
            external_args,
            method_context,
            phantom: _,
        } = unsafe { decoder.decode_next_method_unchecked() }.unwrap();

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
        // SAFETY: Trusted input
        assert!(unsafe { decoder.decode_next_method_unchecked() }.is_none());
    }
}

// TODO: `Vec::push()` in `TransactionPayloadBuilder::push_payload_byte()` is only proven to not
//  panic when the whole payload is constant-folded, which is not the case with multiple method
//  calls, so this test fails `no-panic` build even with much higher inlining threshold
#[cfg(not(feature = "no-panic"))]
#[test]
fn payload_encode_decode_output_index() {
    use crate::payload::builder::tests::ffi::get::DemoContractGetArgs;
    use crate::payload::{FfiDataSizeCapacityRo, FfiDataSizeCapacityRw};

    fn decode_next_method<'a>(
        decoder: &'a mut TransactionPayloadDecoder<'_>,
        verify: bool,
    ) -> Option<PreparedMethod<'a>> {
        if verify {
            decoder.decode_next_method().unwrap()
        } else {
            // SAFETY: Trusted input
            unsafe { decoder.decode_next_method_unchecked() }
        }
    }

    let expected_contract = Address::SYSTEM_SIMPLE_WALLET_BASE;
    let output_value = 42;

    // The output of the first method is used as the input of the second method
    let payload = {
        let mut unused_output = 0;
        let mut builder = TransactionPayloadBuilder::default();
        builder
            .with_method_call(
                &expected_contract,
                &DemoContractGetArgs::new(&mut unused_output),
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

    for verify in [true, false] {
        let mut decoder = TransactionPayloadDecoder::new(
            &payload,
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
            phantom: _,
        } = decode_next_method(&mut decoder, verify).unwrap();

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
            phantom: _,
        } = decode_next_method(&mut decoder, verify).unwrap();

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

        assert!(decode_next_method(&mut decoder, verify).is_none());
    }
}
