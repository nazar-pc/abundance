//! `Alignment::of()` doesn't compile for a type that is not `TrivialType`

use ab_io_type::alignment::Alignment;

struct Align32;

fn main() {
    let _ = Alignment::of::<Align32>();
}
