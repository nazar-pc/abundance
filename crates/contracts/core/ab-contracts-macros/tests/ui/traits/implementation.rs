//! `#[contract]` trait implementation must have exactly the same methods as the trait definition,
//! callers derive method fingerprints from the trait definition, while method attributes are
//! invisible to the compiler

use ab_io_type::trivial_type::TrivialType;

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct Example {
    pub value: u8,
}

mod counter {
    use super::Example;
    use ab_contracts_macros::contract;

    #[contract]
    pub trait Counter {
        #[update]
        fn increment(#[input] value: &Example);
    }
}

mod getter {
    use super::Example;
    use ab_contracts_macros::contract;

    #[contract]
    pub trait Getter {
        #[view]
        fn get(#[input] value: &Example) -> u8;
    }
}

mod kind_mismatch {
    use super::Example;
    use super::counter::Counter;
    use ab_contracts_macros::contract;

    // `#[update]` method of the trait implemented as `#[view]`
    #[contract]
    impl Counter for Example {
        #[view]
        fn increment(#[input] _value: &Example) {}
    }
}

mod missing_attribute {
    use super::Example;
    use super::getter::Getter;
    use ab_contracts_macros::contract;

    // Trait method implemented without `#[view]`, so it is not a contract method
    #[contract]
    impl Getter for Example {
        fn get(_value: &Example) -> u8 {
            0
        }
    }
}

fn main() {}
