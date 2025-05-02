use crate::payload::builder::TransactionPayloadBuilder;
use crate::payload::builder::tests::ffi::set::DemoContractSetArgs;
use crate::payload::{TransactionMethodContext, TransactionPayloadDecoder};
use crate::{EXTERNAL_ARGS_BUFFER_SIZE, OUTPUT_BUFFER_OFFSETS_SIZE, OUTPUT_BUFFER_SIZE};
use ab_contracts_common::Address;
use ab_contracts_common::env::{MethodContext, PreparedMethod};
use ab_contracts_common::method::ExternalArgs;
use ab_contracts_macros::contract;
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
    let mut output_buffer_offsets = [MaybeUninit::uninit(); OUTPUT_BUFFER_OFFSETS_SIZE];

    // Untrusted
    {
        let mut decoder = TransactionPayloadDecoder::new(
            &payload,
            &mut external_args_buffer,
            &mut output_buffer,
            &mut output_buffer_offsets,
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
            unsafe { external_args.read().cast::<u8>().read() },
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
            &mut output_buffer_offsets,
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
        } = unsafe { decoder.decode_next_method_unchecked() }.unwrap();

        assert_eq!(contract, expected_contract);
        assert_eq!(fingerprint, DemoContractSetArgs::FINGERPRINT);
        assert_eq!(
            unsafe { external_args.read().cast::<u8>().read() },
            new_value
        );
        assert_eq!(method_context, MethodContext::Keep);

        // There is some padding, but it is correctly determined that there is no payload anymore
        assert!(!decoder.payload.is_empty());
        assert!(unsafe { decoder.decode_next_method_unchecked() }.is_none());
    }
}
