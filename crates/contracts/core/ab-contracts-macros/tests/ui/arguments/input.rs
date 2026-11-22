//! `#[input]` argument must be a shared reference, inputs are read-only and changes to them are
//! never propagated back to the caller. It must be bound to a simple variable or a pattern like
//! `&value`, since the name is recorded in metadata. `#[tmp]` and `#[slot]` have the same rule,
//! which this case stands for.

mod exclusive_reference {
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
        pub fn set(#[input] _value: &mut u8) {}
    }
}

mod by_value {
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
        pub fn set(#[input] _value: u8) {}
    }
}

mod wildcard_name {
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
        pub fn set(#[input] _: &u8) {}
    }
}

fn main() {}
