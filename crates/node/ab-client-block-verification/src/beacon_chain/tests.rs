use crate::beacon_chain::is_below_archiving_point;
use ab_core_primitives::block::BlockNumber;

#[test]
fn below_archiving_point_matches_fork_pruning_block_offset() {
    for block_confirmation_depth in [2, 5, 100] {
        let best_number = 1_000;
        let is_below = |block_offset| {
            is_below_archiving_point(
                BlockNumber::from(best_number - block_offset),
                BlockNumber::from(best_number),
                BlockNumber::from(block_confirmation_depth),
            )
        };

        // The next best block and forks down to block offset `block_confirmation_depth - 2`
        assert!(
            !is_below_archiving_point(
                BlockNumber::from(best_number + 1),
                BlockNumber::from(best_number),
                BlockNumber::from(block_confirmation_depth),
            ),
            "{block_confirmation_depth}"
        );
        assert!(!is_below(0), "{block_confirmation_depth}");
        assert!(
            !is_below(block_confirmation_depth - 2),
            "{block_confirmation_depth}"
        );
        // Block offset `block_confirmation_depth - 1` and deeper
        assert!(
            is_below(block_confirmation_depth - 1),
            "{block_confirmation_depth}"
        );
        assert!(
            is_below(block_confirmation_depth),
            "{block_confirmation_depth}"
        );
    }
}

#[test]
fn below_archiving_point_does_not_overflow() {
    for block_confirmation_depth in [2, 5, 100] {
        for block_number in [BlockNumber::MAX - BlockNumber::ONE, BlockNumber::MAX] {
            assert!(
                !is_below_archiving_point(
                    block_number,
                    BlockNumber::from(1_000),
                    BlockNumber::from(block_confirmation_depth),
                ),
                "{block_confirmation_depth} {block_number}"
            );
        }
    }
}
