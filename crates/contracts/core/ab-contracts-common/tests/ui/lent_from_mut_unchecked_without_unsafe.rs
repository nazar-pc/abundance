//! `Lent::from_mut_unchecked()` can only be called in `unsafe` code, since the memory it lends is
//! only borrowed for the lifetime of the returned `Lent` and may hold an invalid value afterward

use ab_contracts_common::output::Lent;
use ab_io_type::trivial_type::TrivialType;

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(u8)]
enum Mode {
    A,
    B,
}

fn lend(mode: &mut Mode) -> Lent<'_, Mode> {
    Lent::from_mut_unchecked(mode)
}

fn main() {
    let _ = (lend, Mode::A, Mode::B);
}
