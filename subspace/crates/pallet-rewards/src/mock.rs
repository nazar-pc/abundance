use frame_support::derive_impl;
use frame_support::traits::{ConstU128, ConstU32};
use subspace_runtime_primitives::{ConsensusEventSegmentSize, FindBlockRewardAddress};

type Block = frame_system::mocking::MockBlock<Test>;
type Balance = u128;

frame_support::construct_runtime!(
    pub struct Test {
        System: frame_system,
        Balances: pallet_balances,
        Rewards: crate,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountData = pallet_balances::AccountData<Balance>;
    type EventSegmentSize = ConsensusEventSegmentSize;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig as pallet_balances::DefaultConfig)]
impl pallet_balances::Config for Test {
    type Balance = Balance;
    type ExistentialDeposit = ConstU128<1>;
    type AccountStore = System;
    type RuntimeHoldReason = ();
    type DustRemoval = ();
}

pub struct MockFindBlockRewardAddress;

impl<RewardAddress> FindBlockRewardAddress<RewardAddress> for MockFindBlockRewardAddress {
    fn find_block_reward_address() -> Option<RewardAddress> {
        None
    }
}

impl crate::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type AvgBlockspaceUsageNumBlocks = ConstU32<10>;
    type TransactionByteFee = ConstU128<1>;
    type MaxRewardPoints = ConstU32<20>;
    type FindBlockRewardAddress = MockFindBlockRewardAddress;
    type WeightInfo = ();
    type OnReward = ();
}
