//! `#[contract]` trait definition describes methods that callers can call on any contract that
//! implements the trait. It only declares `#[update]` and `#[view]` methods with `#[env]`,
//! `#[input]` and `#[output]` arguments, since the state and storage of each contract are its own,
//! and each contract provides the bodies.

mod receiver {
    use ab_contracts_macros::contract;

    #[contract]
    pub trait Counter {
        #[update]
        fn increment(&mut self);
    }
}

mod default_body {
    use ab_contracts_macros::contract;

    #[contract]
    pub trait Getter {
        #[view]
        fn get() -> u8 {
            0
        }
    }
}

mod slot {
    use ab_contracts_macros::contract;

    #[contract]
    pub trait Reader {
        #[view]
        fn read(#[slot] slot: &ab_io_type::maybe_data::MaybeData<'_, u8>);
    }
}

fn main() {}
