//! Consensus extension module tests for Subspace consensus.

use crate::mock::{
    create_segment_header, go_to_block, new_test_ext, progress_to_block, RuntimeEvent,
    RuntimeOrigin, Subspace, System, Test, INITIAL_SOLUTION_RANGE, SLOT_PROBABILITY,
};
use crate::{
    pallet, AllowAuthoringByAnyone, Call, Config, PotSlotIterations, PotSlotIterationsValue,
};
use frame_support::{assert_err, assert_ok};
use frame_system::{EventRecord, Phase};
use schnorrkel::Keypair;
use sp_consensus_slots::Slot;
use sp_consensus_subspace::SolutionRanges;
use sp_runtime::traits::BlockNumberProvider;
use sp_runtime::transaction_validity::{
    InvalidTransaction, TransactionPriority, TransactionSource, ValidTransaction,
};
use sp_runtime::DispatchError;
use std::assert_matches::assert_matches;
use std::num::NonZeroU32;
use subspace_core_primitives::segments::SegmentIndex;

#[test]
fn can_update_solution_range_on_era_change() {
    new_test_ext().execute_with(|| {
        let keypair = Keypair::generate();

        assert_eq!(<Test as Config>::ConsensusConstants::get().era_duration, 4);
        let initial_solution_ranges = SolutionRanges {
            current: INITIAL_SOLUTION_RANGE,
            next: None,
        };
        assert_eq!(Subspace::solution_ranges(), initial_solution_ranges);
        // enable solution range adjustment
        assert_ok!(Subspace::enable_solution_range_adjustment(
            RuntimeOrigin::root(),
            None
        ));

        // Progress to almost era edge
        progress_to_block(&keypair, 3);
        // No solution range update
        assert_eq!(Subspace::solution_ranges(), initial_solution_ranges);

        // Era edge
        progress_to_block(&keypair, 4);
        // Next solution range should be updated, but current is still unchanged
        let updated_solution_ranges = Subspace::solution_ranges();
        assert_eq!(
            updated_solution_ranges.current,
            initial_solution_ranges.current
        );
        assert!(updated_solution_ranges.next.is_some());

        progress_to_block(&keypair, 5);
        // Next solution range should become current
        assert_eq!(
            Subspace::solution_ranges(),
            SolutionRanges {
                current: updated_solution_ranges.next.unwrap(),
                next: None,
            }
        );

        // Because blocks were produced on every slot, apparent pledged space must increase and
        // solution range should decrease
        let last_solution_range = Subspace::solution_ranges().current;
        assert!(last_solution_range < INITIAL_SOLUTION_RANGE);

        // Progress to era edge such that it takes more slots than expected
        go_to_block(
            &keypair,
            8,
            u64::from(Subspace::current_slot())
                + (4 * SLOT_PROBABILITY.1 / SLOT_PROBABILITY.0 + 10),
        );
        // This should cause solution range to increase as apparent pledged space decreased
        assert!(Subspace::solution_ranges().next.unwrap() > last_solution_range);
    })
}

#[test]
fn can_override_solution_range_update() {
    new_test_ext().execute_with(|| {
        let keypair = Keypair::generate();

        let initial_solution_ranges = SolutionRanges {
            current: INITIAL_SOLUTION_RANGE,
            next: None,
        };
        assert_eq!(Subspace::solution_ranges(), initial_solution_ranges);
        // enable solution range adjustment
        let random_solution_range = rand::random();
        assert_ok!(Subspace::enable_solution_range_adjustment(
            RuntimeOrigin::root(),
            Some(random_solution_range),
        ));

        // Solution range must be updated instantly
        let updated_solution_ranges = Subspace::solution_ranges();
        assert_eq!(updated_solution_ranges.current, random_solution_range);

        // Era edge
        progress_to_block(
            &keypair,
            <Test as Config>::ConsensusConstants::get().era_duration,
        );
        // Next solution range should be updated to the same value as current due to override
        let updated_solution_ranges = Subspace::solution_ranges();
        assert_eq!(updated_solution_ranges.current, random_solution_range);
        assert_eq!(updated_solution_ranges.next, Some(random_solution_range));
    })
}

#[test]
fn solution_range_should_not_update_when_disabled() {
    new_test_ext().execute_with(|| {
        let keypair = Keypair::generate();

        assert_eq!(<Test as Config>::ConsensusConstants::get().era_duration, 4);
        let initial_solution_ranges = SolutionRanges {
            current: INITIAL_SOLUTION_RANGE,
            next: None,
        };
        assert_eq!(Subspace::solution_ranges(), initial_solution_ranges);

        // Progress to almost era edge
        progress_to_block(&keypair, 3);
        // No solution range update
        assert_eq!(Subspace::solution_ranges(), initial_solution_ranges);

        // Era edge
        progress_to_block(&keypair, 4);
        // Next solution range should be updated, but current is still unchanged
        let updated_solution_ranges = Subspace::solution_ranges();
        assert_eq!(
            updated_solution_ranges.current,
            initial_solution_ranges.current
        );
        assert!(updated_solution_ranges.next.is_some());

        progress_to_block(&keypair, 5);
        // Next solution range should become current
        assert_eq!(
            Subspace::solution_ranges(),
            SolutionRanges {
                current: updated_solution_ranges.next.unwrap(),
                next: None,
            }
        );

        // since solution range adjustment was disabled, solution range will remain the same
        let last_solution_range = Subspace::solution_ranges().current;
        assert_eq!(last_solution_range, INITIAL_SOLUTION_RANGE);

        // Progress to era edge such that it takes more slots than expected
        go_to_block(
            &keypair,
            8,
            u64::from(Subspace::current_slot())
                + (4 * SLOT_PROBABILITY.1 / SLOT_PROBABILITY.0 + 10),
        );
        // Solution rage will still be the same even after the apparent pledged space has decreased
        // since adjustment is disabled
        assert_eq!(
            Subspace::solution_ranges().next.unwrap(),
            INITIAL_SOLUTION_RANGE
        );
    })
}

#[test]
fn store_segment_header_works() {
    new_test_ext().execute_with(|| {
        let keypair = Keypair::generate();

        progress_to_block(&keypair, 1);

        let segment_header = create_segment_header(SegmentIndex::ZERO);

        Subspace::store_segment_headers(RuntimeOrigin::none(), vec![segment_header]).unwrap();
        assert_eq!(
            System::events(),
            vec![EventRecord {
                phase: Phase::Initialization,
                event: RuntimeEvent::Subspace(crate::Event::SegmentHeaderStored { segment_header }),
                topics: vec![],
            }]
        );
    });
}

#[test]
fn store_segment_header_validate_unsigned_prevents_duplicates() {
    new_test_ext().execute_with(|| {
        let keypair = Keypair::generate();

        progress_to_block(&keypair, 1);

        let segment_header = create_segment_header(SegmentIndex::ZERO);

        let inner = Call::store_segment_headers {
            segment_headers: vec![segment_header],
        };

        // Only local/in block reports are allowed
        assert_eq!(
            <Subspace as sp_runtime::traits::ValidateUnsigned>::validate_unsigned(
                TransactionSource::External,
                &inner,
            ),
            InvalidTransaction::Call.into(),
        );

        // The transaction is valid when passed as local
        assert_eq!(
            <Subspace as sp_runtime::traits::ValidateUnsigned>::validate_unsigned(
                TransactionSource::Local,
                &inner,
            ),
            Ok(ValidTransaction {
                priority: TransactionPriority::MAX,
                requires: vec![],
                provides: vec![],
                longevity: 0,
                propagate: false,
            })
        );

        // The pre dispatch checks should also pass
        assert_ok!(<Subspace as sp_runtime::traits::ValidateUnsigned>::pre_dispatch(&inner));

        // Submit the report
        Subspace::store_segment_headers(RuntimeOrigin::none(), vec![segment_header]).unwrap();

        // The report should now be considered stale and the transaction is invalid.
        // The check for staleness should be done on both `validate_unsigned` and on `pre_dispatch`
        assert_err!(
            <Subspace as sp_runtime::traits::ValidateUnsigned>::validate_unsigned(
                TransactionSource::Local,
                &inner,
            ),
            InvalidTransaction::BadMandatory,
        );
        assert_err!(
            <Subspace as sp_runtime::traits::ValidateUnsigned>::pre_dispatch(&inner),
            InvalidTransaction::BadMandatory,
        );

        let inner2 = Call::store_segment_headers {
            segment_headers: vec![
                create_segment_header(SegmentIndex::ONE),
                create_segment_header(SegmentIndex::ONE),
            ],
        };

        // Same segment header can't be included twice even in the same extrinsic
        assert_err!(
            <Subspace as sp_runtime::traits::ValidateUnsigned>::validate_unsigned(
                TransactionSource::Local,
                &inner2,
            ),
            InvalidTransaction::BadMandatory,
        );
        assert_err!(
            <Subspace as sp_runtime::traits::ValidateUnsigned>::pre_dispatch(&inner2),
            InvalidTransaction::BadMandatory,
        );
    });
}

#[test]
fn allow_authoring_by_anyone_works() {
    new_test_ext().execute_with(|| {
        let keypair1 = Keypair::generate();
        let keypair2 = Keypair::generate();

        // By default block authoring is allowed by the pallet
        progress_to_block(
            &keypair1,
            frame_system::Pallet::<Test>::current_block_number() + 1,
        );
        progress_to_block(
            &keypair2,
            frame_system::Pallet::<Test>::current_block_number() + 1,
        );

        // Disable default behavior
        AllowAuthoringByAnyone::<Test>::put(false);
        // First author can produce blocks
        progress_to_block(
            &keypair1,
            frame_system::Pallet::<Test>::current_block_number() + 1,
        );
        progress_to_block(
            &keypair1,
            frame_system::Pallet::<Test>::current_block_number() + 1,
        );
        // However authoring with a different public key panics (client error)
        assert!(std::panic::catch_unwind(|| {
            progress_to_block(
                &keypair2,
                frame_system::Pallet::<Test>::current_block_number() + 1,
            );
        })
        .is_err());

        // Unlock authoring by anyone
        assert_err!(
            Subspace::enable_authoring_by_anyone(RuntimeOrigin::signed(1)),
            DispatchError::BadOrigin
        );
        Subspace::enable_authoring_by_anyone(RuntimeOrigin::root()).unwrap();
        // Both must be able to create blocks again
        progress_to_block(
            &keypair1,
            frame_system::Pallet::<Test>::current_block_number() + 1,
        );
        progress_to_block(
            &keypair2,
            frame_system::Pallet::<Test>::current_block_number() + 1,
        );
    });
}

#[test]
fn set_pot_slot_iterations_works() {
    new_test_ext().execute_with(|| {
        PotSlotIterations::<Test>::put(PotSlotIterationsValue {
            slot_iterations: NonZeroU32::new(100_000_000).unwrap(),
            update: None,
        });

        // Only root can do this
        assert_err!(
            Subspace::set_pot_slot_iterations(
                RuntimeOrigin::signed(1),
                NonZeroU32::new(100_000_000).unwrap()
            ),
            DispatchError::BadOrigin
        );

        // Must increase
        assert_matches!(
            Subspace::set_pot_slot_iterations(
                RuntimeOrigin::root(),
                NonZeroU32::new(100_000_000).unwrap()
            ),
            Err(DispatchError::Module(_))
        );

        // Must be multiple of PotCheckpoints iterations times two
        assert_matches!(
            Subspace::set_pot_slot_iterations(
                RuntimeOrigin::root(),
                NonZeroU32::new(100_000_001).unwrap()
            ),
            Err(DispatchError::Module(_))
        );

        // Now it succeeds
        Subspace::set_pot_slot_iterations(
            RuntimeOrigin::root(),
            NonZeroU32::new(110_000_000).unwrap(),
        )
        .unwrap();

        // Subsequent calls succeed too
        Subspace::set_pot_slot_iterations(
            RuntimeOrigin::root(),
            NonZeroU32::new(120_000_000).unwrap(),
        )
        .unwrap();

        // Unless update is already scheduled to be applied
        pallet::PotSlotIterations::<Test>::mutate(|pot_slot_iterations| {
            pot_slot_iterations
                .as_mut()
                .unwrap()
                .update
                .as_mut()
                .unwrap()
                .target_slot
                .replace(Slot::from(1));
        });
        assert_matches!(
            Subspace::set_pot_slot_iterations(
                RuntimeOrigin::root(),
                NonZeroU32::new(130_000_000).unwrap()
            ),
            Err(DispatchError::Module(_))
        );
    });
}
