//! `#[output]` argument must be an exclusive reference bound to a simple variable. Current
//! contents of outputs provided by the caller can be both read and modified by the contract, and
//! changes are propagated back to the caller. The macro needs a name for the output, and unlike
//! `#[input]`, patterns like `&mut value` are rejected, since they would not allow modifying the
//! output.

mod shared_reference {
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
        pub fn get(#[output] _value: &u8) {}
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
        pub fn get(#[output] _value: u8) {}
    }
}

mod wildcard {
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
        pub fn get(#[output] _: &mut u8) {}
    }
}

mod tuple {
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
        pub fn get(#[output] (_first, _second): &mut (u8, u8)) {}
    }
}

mod reference_pattern {
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
        pub fn get(#[output] &mut _value: &mut u8) {}
    }
}

fn main() {}
