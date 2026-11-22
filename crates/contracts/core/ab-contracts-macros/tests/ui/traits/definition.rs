//! `#[contract]` trait definition describes methods that callers can call on any contract that
//! implements the trait. It only declares `#[update]` and `#[view]` methods with `#[env]`,
//! `#[input]` and `#[output]` arguments, since the state and storage of each contract are its own.

mod slot {
    use ab_contracts_macros::contract;

    #[contract]
    pub trait Reader {
        #[view]
        fn read(#[slot] slot: &ab_io_type::maybe_data::MaybeData<'_, u8>);
    }
}

fn main() {}
