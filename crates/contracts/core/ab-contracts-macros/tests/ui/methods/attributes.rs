//! A method of `#[contract]` is marked with exactly one of `#[init]`, `#[update]` or `#[view]`. In
//! a struct implementation such a method must be public, which trait implementations don't need,
//! since their methods are public through the trait. It can't have a custom ABI either.

mod two_kinds {
    use ab_contracts_macros::contract;
    use ab_io_type::trivial_type::TrivialType;

    #[derive(Debug, Copy, Clone, TrivialType)]
    #[repr(C)]
    pub struct Example {
        pub value: u8,
    }

    #[contract]
    impl Example {
        #[update]
        #[view]
        pub fn get(&self) -> u8 {
            self.value
        }
    }
}

mod not_public {
    use ab_contracts_macros::contract;
    use ab_io_type::trivial_type::TrivialType;

    #[derive(Debug, Copy, Clone, TrivialType)]
    #[repr(C)]
    pub struct Example {
        pub value: u8,
    }

    #[contract]
    impl Example {
        #[view]
        fn get(&self) -> u8 {
            self.value
        }
    }
}

mod custom_abi {
    use ab_contracts_macros::contract;
    use ab_io_type::trivial_type::TrivialType;

    #[derive(Debug, Copy, Clone, TrivialType)]
    #[repr(C)]
    pub struct Example {
        pub value: u8,
    }

    #[contract]
    impl Example {
        #[view]
        pub extern "C" fn get(&self) -> u8 {
            self.value
        }
    }
}

fn main() {}
