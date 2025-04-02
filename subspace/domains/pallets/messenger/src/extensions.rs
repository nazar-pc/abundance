//! Extensions for unsigned general extrinsics

use crate::pallet::Call as MessengerCall;
use crate::{
    Call, Config, Origin, Pallet as Messenger, ValidatedRelayMessage, XDM_TRANSACTION_LONGEVITY,
};
use core::cmp::Ordering;
use frame_support::pallet_prelude::{PhantomData, TypeInfo};
use frame_support::RuntimeDebugNoBound;
use frame_system::pallet_prelude::RuntimeCallFor;
use parity_scale_codec::{Decode, Encode};
use scale_info::prelude::fmt;
use sp_messenger::messages::{Message, Nonce};
use sp_messenger::MAX_FUTURE_ALLOWED_NONCES;
use sp_runtime::impl_tx_ext_default;
use sp_runtime::traits::{
    AsSystemOriginSigner, DispatchInfoOf, DispatchOriginOf, Dispatchable, Implication,
    TransactionExtension, ValidateResult,
};
use sp_runtime::transaction_validity::{
    InvalidTransaction, TransactionSource, TransactionValidityError, ValidTransaction,
    ValidTransactionBuilder,
};
use sp_subspace_mmr::MmrProofVerifier;

/// Trait to convert Runtime call to possible Messenger call.
pub trait MaybeMessengerCall<Runtime>
where
    Runtime: Config,
{
    fn maybe_messenger_call(&self) -> Option<&MessengerCall<Runtime>>;
}

/// Data passed from validate to prepare.
#[derive(RuntimeDebugNoBound)]
pub enum Val<T: Config + fmt::Debug> {
    /// No validation data
    None,
    /// Validated data
    ValidatedRelayMessage(ValidatedRelayMessage<T>),
}

/// Extensions for pallet-messenger unsigned extrinsics.
#[derive(Encode, Decode, Clone, Eq, PartialEq, TypeInfo)]
pub struct MessengerExtension<Runtime>(PhantomData<Runtime>);

impl<Runtime> MessengerExtension<Runtime> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<Runtime> Default for MessengerExtension<Runtime> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Config> fmt::Debug for MessengerExtension<T> {
    #[cfg(feature = "std")]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MessengerExtension",)
    }

    #[cfg(not(feature = "std"))]
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}

impl<Runtime> MessengerExtension<Runtime>
where
    Runtime: Config + scale_info::TypeInfo + fmt::Debug + Send + Sync,
{
    fn check_future_nonce_and_add_requires(
        mut valid_tx_builder: ValidTransactionBuilder,
        validated_relay_message: &ValidatedRelayMessage<Runtime>,
    ) -> Result<ValidTransactionBuilder, TransactionValidityError> {
        let Message {
            dst_chain_id,
            channel_id,
            nonce: msg_nonce,
            ..
        } = &validated_relay_message.message;

        let next_nonce = validated_relay_message.next_nonce;
        // Only add the requires tag if the msg nonce is in future
        if *msg_nonce > next_nonce {
            let max_future_nonce = next_nonce.saturating_add(MAX_FUTURE_ALLOWED_NONCES.into());
            if *msg_nonce > max_future_nonce {
                return Err(InvalidTransaction::Custom(
                    crate::verification_errors::IN_FUTURE_NONCE,
                )
                .into());
            }

            valid_tx_builder =
                valid_tx_builder.and_requires((dst_chain_id, channel_id, msg_nonce - Nonce::one()));
        };

        Ok(valid_tx_builder)
    }

    fn do_validate(
        call: &MessengerCall<Runtime>,
    ) -> Result<(ValidTransaction, ValidatedRelayMessage<Runtime>), TransactionValidityError> {
        match call {
            Call::relay_message { msg: xdm } => {
                let consensus_state_root =
                    Runtime::MmrProofVerifier::verify_proof_and_extract_leaf(
                        xdm.proof.consensus_mmr_proof(),
                    )
                    .ok_or(InvalidTransaction::BadProof)?
                    .state_root();

                let validated_message =
                    Messenger::<Runtime>::validate_relay_message(xdm, consensus_state_root)?;

                let Message {
                    dst_chain_id,
                    channel_id,
                    nonce: msg_nonce,
                    ..
                } = &validated_message.message;

                let valid_tx_builder = Self::check_future_nonce_and_add_requires(
                    ValidTransaction::with_tag_prefix("MessengerInbox"),
                    &validated_message,
                )?;

                let validity = valid_tx_builder
                    // XDM have a bit higher priority than normal extrinsic but must less than
                    // fraud proof
                    .priority(1)
                    .longevity(XDM_TRANSACTION_LONGEVITY)
                    .and_provides((dst_chain_id, channel_id, msg_nonce))
                    .propagate(true)
                    .build()?;

                Ok((validity, validated_message))
            }
            Call::relay_message_response { msg: xdm } => {
                let consensus_state_root =
                    Runtime::MmrProofVerifier::verify_proof_and_extract_leaf(
                        xdm.proof.consensus_mmr_proof(),
                    )
                    .ok_or(InvalidTransaction::BadProof)?
                    .state_root();

                let validated_message = Messenger::<Runtime>::validate_relay_message_response(
                    xdm,
                    consensus_state_root,
                )?;

                let Message {
                    dst_chain_id,
                    channel_id,
                    nonce: msg_nonce,
                    ..
                } = &validated_message.message;

                let valid_tx_builder = Self::check_future_nonce_and_add_requires(
                    ValidTransaction::with_tag_prefix("MessengerOutboxResponse"),
                    &validated_message,
                )?;

                let validity = valid_tx_builder
                    // XDM have a bit higher priority than normal extrinsic but must less than
                    // fraud proof
                    .priority(1)
                    .longevity(XDM_TRANSACTION_LONGEVITY)
                    .and_provides((dst_chain_id, channel_id, msg_nonce))
                    .propagate(true)
                    .build()?;

                Ok((validity, validated_message))
            }
            _ => Err(InvalidTransaction::Call.into()),
        }
    }

    fn do_prepare(
        call: &MessengerCall<Runtime>,
        val: ValidatedRelayMessage<Runtime>,
    ) -> Result<(), TransactionValidityError> {
        let ValidatedRelayMessage {
            message,
            should_init_channel,
            next_nonce,
        } = val;

        // Reject in future message
        if message.nonce.cmp(&next_nonce) == Ordering::Greater {
            return Err(InvalidTransaction::Future.into());
        }

        match call {
            Call::relay_message { .. } => {
                Messenger::<Runtime>::pre_dispatch_relay_message(message, should_init_channel)
            }
            Call::relay_message_response { .. } => {
                Messenger::<Runtime>::pre_dispatch_relay_message_response(message)
            }
            _ => Err(InvalidTransaction::Call.into()),
        }
    }
}

impl<Runtime> TransactionExtension<RuntimeCallFor<Runtime>> for MessengerExtension<Runtime>
where
    Runtime: Config + scale_info::TypeInfo + fmt::Debug + Send + Sync,
    <RuntimeCallFor<Runtime> as Dispatchable>::RuntimeOrigin:
        AsSystemOriginSigner<<Runtime as frame_system::Config>::AccountId> + From<Origin> + Clone,
    RuntimeCallFor<Runtime>: MaybeMessengerCall<Runtime>,
{
    const IDENTIFIER: &'static str = "MessengerExtension";
    type Implicit = ();
    type Val = Val<Runtime>;
    type Pre = ();

    fn validate(
        &self,
        origin: DispatchOriginOf<RuntimeCallFor<Runtime>>,
        call: &RuntimeCallFor<Runtime>,
        _info: &DispatchInfoOf<RuntimeCallFor<Runtime>>,
        _len: usize,
        _self_implicit: Self::Implicit,
        _inherited_implication: &impl Implication,
        _source: TransactionSource,
    ) -> ValidateResult<Self::Val, RuntimeCallFor<Runtime>> {
        // we only care about unsigned calls
        if origin.as_system_origin_signer().is_some() {
            return Ok((ValidTransaction::default(), Val::None, origin));
        };

        let messenger_call = match call.maybe_messenger_call() {
            Some(messenger_call) => messenger_call,
            None => return Ok((ValidTransaction::default(), Val::None, origin)),
        };

        let (validity, validated_relay_message) = Self::do_validate(messenger_call)?;
        Ok((
            validity,
            Val::ValidatedRelayMessage(validated_relay_message),
            Origin::ValidatedUnsigned.into(),
        ))
    }

    fn prepare(
        self,
        val: Self::Val,
        _origin: &DispatchOriginOf<RuntimeCallFor<Runtime>>,
        call: &RuntimeCallFor<Runtime>,
        _info: &DispatchInfoOf<RuntimeCallFor<Runtime>>,
        _len: usize,
    ) -> Result<Self::Pre, TransactionValidityError> {
        match (call.maybe_messenger_call(), val) {
            // prepare if this is a messenger call and has been validated
            (Some(messenger_call), Val::ValidatedRelayMessage(validated_relay_message)) => {
                Self::do_prepare(messenger_call, validated_relay_message)
            }
            // return Ok for the rest of the call types
            (_, _) => Ok(()),
        }
    }

    // TODO: need benchmarking for this extension.
    impl_tx_ext_default!(RuntimeCallFor<Runtime>; weight);
}

/// Extensions for pallet-messenger unsigned extrinsics with trusted MMR verification.
#[derive(Encode, Decode, Clone, Eq, PartialEq, TypeInfo)]
pub struct MessengerTrustedMmrExtension<Runtime>(PhantomData<Runtime>);

impl<Runtime> MessengerTrustedMmrExtension<Runtime> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<Runtime> Default for MessengerTrustedMmrExtension<Runtime> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Config> fmt::Debug for MessengerTrustedMmrExtension<T> {
    #[cfg(feature = "std")]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MessengerTrustedMmrExtension",)
    }

    #[cfg(not(feature = "std"))]
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}

impl<Runtime> MessengerTrustedMmrExtension<Runtime>
where
    Runtime: Config + scale_info::TypeInfo + fmt::Debug + Send + Sync,
{
    fn do_validate(
        call: &MessengerCall<Runtime>,
    ) -> Result<(ValidTransaction, ValidatedRelayMessage<Runtime>), TransactionValidityError> {
        match call {
            Call::relay_message { msg: xdm } => {
                let consensus_state_root =
                    Runtime::MmrProofVerifier::extract_leaf_without_verifying(
                        xdm.proof.consensus_mmr_proof(),
                    )
                    .ok_or(InvalidTransaction::BadProof)?
                    .state_root();

                let validated_relay_message =
                    Messenger::<Runtime>::validate_relay_message(xdm, consensus_state_root)?;

                Ok((ValidTransaction::default(), validated_relay_message))
            }
            Call::relay_message_response { msg: xdm } => {
                let consensus_state_root =
                    Runtime::MmrProofVerifier::extract_leaf_without_verifying(
                        xdm.proof.consensus_mmr_proof(),
                    )
                    .ok_or(InvalidTransaction::BadProof)?
                    .state_root();

                let validated_relay_message =
                    Messenger::<Runtime>::validate_relay_message_response(
                        xdm,
                        consensus_state_root,
                    )?;

                Ok((ValidTransaction::default(), validated_relay_message))
            }
            _ => Err(InvalidTransaction::Call.into()),
        }
    }
}

impl<Runtime> TransactionExtension<RuntimeCallFor<Runtime>>
    for MessengerTrustedMmrExtension<Runtime>
where
    Runtime: Config + scale_info::TypeInfo + fmt::Debug + Send + Sync,
    <RuntimeCallFor<Runtime> as Dispatchable>::RuntimeOrigin:
        AsSystemOriginSigner<<Runtime as frame_system::Config>::AccountId> + From<Origin> + Clone,
    RuntimeCallFor<Runtime>: MaybeMessengerCall<Runtime>,
{
    const IDENTIFIER: &'static str = "MessengerTrustedMmrExtension";
    type Implicit = ();
    type Val = Val<Runtime>;
    type Pre = ();

    // TODO: need benchmarking for this extension.
    impl_tx_ext_default!(RuntimeCallFor<Runtime>; weight);

    fn validate(
        &self,
        origin: DispatchOriginOf<RuntimeCallFor<Runtime>>,
        call: &RuntimeCallFor<Runtime>,
        _info: &DispatchInfoOf<RuntimeCallFor<Runtime>>,
        _len: usize,
        _self_implicit: Self::Implicit,
        _inherited_implication: &impl Implication,
        _source: TransactionSource,
    ) -> ValidateResult<Self::Val, RuntimeCallFor<Runtime>> {
        // we only care about unsigned calls
        if origin.as_system_origin_signer().is_some() {
            return Ok((ValidTransaction::default(), Val::None, origin));
        };

        let messenger_call = match call.maybe_messenger_call() {
            Some(messenger_call) => messenger_call,
            None => return Ok((ValidTransaction::default(), Val::None, origin)),
        };

        let (validity, validated_relay_message) = Self::do_validate(messenger_call)?;
        Ok((
            validity,
            Val::ValidatedRelayMessage(validated_relay_message),
            Origin::ValidatedUnsigned.into(),
        ))
    }

    fn prepare(
        self,
        val: Self::Val,
        _origin: &DispatchOriginOf<RuntimeCallFor<Runtime>>,
        call: &RuntimeCallFor<Runtime>,
        _info: &DispatchInfoOf<RuntimeCallFor<Runtime>>,
        _len: usize,
    ) -> Result<Self::Pre, TransactionValidityError> {
        match (call.maybe_messenger_call(), val) {
            // prepare if this is a messenger call and has been validated
            (Some(messenger_call), Val::ValidatedRelayMessage(validated_relay_message)) => {
                MessengerExtension::<Runtime>::do_prepare(messenger_call, validated_relay_message)
            }
            // return Ok for the rest of the call types
            (_, _) => Ok(()),
        }
    }
}
