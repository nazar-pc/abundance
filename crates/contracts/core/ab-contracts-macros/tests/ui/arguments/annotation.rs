//! Each argument of a method, except the state, must be annotated with exactly one of `#[env]`,
//! `#[tmp]`, `#[slot]`, `#[input]` or `#[output]`, which determines how it is passed to the method
//! and how it appears in metadata.

mod missing {
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
        pub fn set(_value: &u8) {}
    }
}

mod two_attributes {
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
        pub fn set(
            #[input]
            #[output]
            _value: &u8,
        ) {
        }
    }
}

fn main() {}
