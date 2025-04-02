//! Pallet for tweaking the runtime configs for multiple network.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
pub mod weights;

pub use pallet::*;

#[frame_support::pallet]
mod pallet {
    use crate::weights::WeightInfo;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::Zero;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Whether to enable dynamic cost of storage.
    #[pallet::storage]
    #[pallet::getter(fn enable_dynamic_cost_of_storage)]
    pub type EnableDynamicCostOfStorage<T> = StorageValue<_, bool, ValueQuery>;

    /// Whether to enable balances transfers.
    #[pallet::storage]
    #[pallet::getter(fn enable_balance_transfers)]
    pub type EnableBalanceTransfers<T> = StorageValue<_, bool, ValueQuery>;

    #[pallet::storage]
    pub type ConfirmationDepthK<T: Config> = StorageValue<_, BlockNumberFor<T>, ValueQuery>;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        /// Whether to enable dynamic cost of storage (if `false` cost per byte is equal to 1)
        pub enable_dynamic_cost_of_storage: bool,
        /// Whether to enable balance transfers
        pub enable_balance_transfers: bool,
        /// Confirmation depth k to use in the archiving process
        pub confirmation_depth_k: BlockNumberFor<T>,
    }

    impl<T: Config> Default for GenesisConfig<T> {
        #[inline]
        fn default() -> Self {
            Self {
                enable_dynamic_cost_of_storage: false,
                enable_balance_transfers: false,
                confirmation_depth_k: BlockNumberFor::<T>::from(100u32),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            let Self {
                enable_dynamic_cost_of_storage,
                enable_balance_transfers,
                confirmation_depth_k,
            } = self;

            assert!(
                !confirmation_depth_k.is_zero(),
                "ConfirmationDepthK can not be zero"
            );

            <EnableDynamicCostOfStorage<T>>::put(enable_dynamic_cost_of_storage);
            <EnableBalanceTransfers<T>>::put(enable_balance_transfers);
            <ConfirmationDepthK<T>>::put(confirmation_depth_k);
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Enable or disable dynamic cost of storage.
        #[pallet::call_index(1)]
        #[pallet::weight(< T as Config >::WeightInfo::set_enable_dynamic_cost_of_storage())]
        pub fn set_enable_dynamic_cost_of_storage(
            origin: OriginFor<T>,
            enable_dynamic_cost_of_storage: bool,
        ) -> DispatchResult {
            ensure_root(origin)?;

            EnableDynamicCostOfStorage::<T>::put(enable_dynamic_cost_of_storage);

            Ok(())
        }

        /// Enable or disable balance transfers for all users.
        #[pallet::call_index(2)]
        #[pallet::weight(< T as Config >::WeightInfo::set_enable_balance_transfers())]
        pub fn set_enable_balance_transfers(
            origin: OriginFor<T>,
            enable_balance_transfers: bool,
        ) -> DispatchResult {
            ensure_root(origin)?;

            EnableBalanceTransfers::<T>::put(enable_balance_transfers);

            Ok(())
        }
    }
}
