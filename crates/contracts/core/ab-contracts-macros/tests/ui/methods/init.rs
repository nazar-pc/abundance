//! `#[init]` method must return `Self` or, without a return value, have `Self` as the last
//! `#[output]` argument, which the host stores as the initial state of the contract. If there is a
//! return value, it must be `Self`, even when the last `#[output]` argument is `Self`. There is no
//! state before `#[init]`, so it can't take `self`.

mod without_state {
    use ab_contracts_macros::contract;
    use ab_io_type::trivial_type::TrivialType;

    #[derive(Debug, Copy, Clone, TrivialType)]
    #[repr(C)]
    pub struct Example {
        pub value: u8,
    }

    #[contract]
    impl Example {
        #[init]
        pub fn new() -> u8 {
            0
        }
    }
}

mod return_with_state_output {
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
        pub fn new(#[output] state: &mut ab_io_type::maybe_data::MaybeData<'_, Self>) -> u8 {
            state.replace(Self { value: 1 });
            0
        }
    }
}

mod receiver {
    use ab_contracts_macros::contract;
    use ab_io_type::trivial_type::TrivialType;

    #[derive(Debug, Copy, Clone, TrivialType)]
    #[repr(C)]
    pub struct Example {
        pub value: u8,
    }

    #[contract]
    impl Example {
        #[init]
        pub fn new(&self) -> Self {
            *self
        }
    }
}

fn main() {}
