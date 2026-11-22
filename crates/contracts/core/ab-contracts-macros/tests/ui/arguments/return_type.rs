//! A method returns nothing, `T` or `Result<T, ContractError>`. The caller allocates memory for
//! the return value according to metadata and only knows how to interpret `ContractError`, so other
//! error types are rejected. Some types, like tuples, are not supported as `T` either.

mod result_with_other_error {
    use ab_contracts_macros::contract;
    use ab_io_type::trivial_type::TrivialType;

    #[derive(Debug, Copy, Clone, TrivialType)]
    #[repr(C)]
    pub struct Example {
        pub value: u8,
    }

    pub struct OtherError;

    #[contract]
    impl Example {
        #[view]
        pub fn get() -> Result<u8, OtherError> {
            Ok(0)
        }
    }
}

mod unsupported_type {
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
        pub fn get() -> (u8, u8) {
            (0, 0)
        }
    }
}

fn main() {}
