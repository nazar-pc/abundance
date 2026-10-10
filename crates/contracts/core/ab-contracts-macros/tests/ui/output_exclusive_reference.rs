//! `#[output]` argument must be an exclusive reference, current contents of outputs provided by the
//! caller can be both read and modified by the contract, and changes are propagated back to the
//! caller

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

fn main() {}
