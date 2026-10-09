//! The host stores the return value of `#[init]` as the initial state of the contract, so if there
//! is a return value, it must be `Self`, even when the last `#[output]` argument is `Self`

mod regular {
    use ab_contracts_macros::contract;
    use ab_io_type::trivial_type::TrivialType;

    #[derive(Debug, Copy, Clone, TrivialType)]
    #[repr(C)]
    pub struct Example {
        pub value: u64,
    }

    #[contract]
    impl Example {
        #[init]
        pub fn new(#[output] state: &mut ab_io_type::maybe_data::MaybeData<Self>) -> u8 {
            state.replace(Self { value: 1 });
            0
        }
    }
}

mod result {
    use ab_contracts_macros::contract;
    use ab_io_type::trivial_type::TrivialType;

    #[derive(Debug, Copy, Clone, TrivialType)]
    #[repr(C)]
    pub struct Example {
        pub value: u64,
    }

    #[contract]
    impl Example {
        #[init]
        pub fn new(
            #[output] state: &mut ab_io_type::maybe_data::MaybeData<Self>,
        ) -> Result<u8, ab_contracts_common::ContractError> {
            state.replace(Self { value: 1 });
            Ok(0)
        }
    }
}

fn main() {}
