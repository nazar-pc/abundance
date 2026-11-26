use crate::single_disk_farm::plotting::select_history_size;
use ab_core_primitives::segments::HistorySize;
use std::num::NonZeroU64;

const MIN_SECTOR_LIFETIME: u64 = 4;

fn history_size(value: u64) -> HistorySize {
    HistorySize::new(NonZeroU64::new(value).unwrap())
}

fn select(
    max_used_history_size: Option<u64>,
    old_sector_history_size: Option<u64>,
    current_history_size: u64,
) -> HistorySize {
    select_history_size(
        max_used_history_size.map(history_size),
        old_sector_history_size.map(history_size),
        history_size(current_history_size),
        history_size(MIN_SECTOR_LIFETIME),
    )
}

#[test]
fn no_sectors_plotted_yet() {
    assert_eq!(select(None, None, 1), history_size(1));
    assert_eq!(select(None, None, 100), history_size(100));
}

#[test]
fn recent_max_used_history_size() {
    assert_eq!(select(Some(100), None, 100), history_size(100));
    assert_eq!(select(Some(100), None, 101), history_size(100));
    assert_eq!(select(Some(100), Some(90), 101), history_size(100));
}

#[test]
fn old_max_used_history_size() {
    assert_eq!(select(Some(1), None, 100), history_size(100));
    assert_eq!(select(Some(50), Some(40), 100), history_size(100));
}

#[test]
fn replotting_at_max_used_history_size() {
    assert_eq!(select(Some(100), Some(100), 101), history_size(101));
}

#[test]
fn max_used_history_size_expiration_boundary() {
    // The earliest expiration of a sector plotted at history size 100 is 104, such a sector is
    // still plotted at 102, but would already be about to expire at 103
    assert_eq!(select(Some(100), None, 102), history_size(100));
    assert_eq!(select(Some(100), None, 103), history_size(103));
}
