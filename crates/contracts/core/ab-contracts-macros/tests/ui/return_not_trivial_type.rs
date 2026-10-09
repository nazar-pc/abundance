//! Return values are written by the callee into memory allocated for them by the caller according
//! to metadata, which is only possible for `TrivialType`, other types must use `#[output]`

mod method {
    use ab_contracts_macros::contract;
    use ab_io_type::trivial_type::TrivialType;
    use ab_io_type::variable_bytes::VariableBytes;

    #[derive(Debug, Copy, Clone, TrivialType)]
    #[repr(C)]
    pub struct Example {
        pub value: u8,
    }

    #[contract]
    impl Example {
        #[view]
        pub fn get(&self) -> VariableBytes<0> {
            unimplemented!()
        }
    }
}

mod trait_definition {
    use ab_contracts_common::env::Env;
    use ab_contracts_macros::contract;
    use ab_io_type::variable_bytes::VariableBytes;

    #[contract]
    pub trait Getter {
        #[view]
        fn get(#[env] env: &Env<'_>) -> VariableBytes<0>;
    }
}

fn main() {}
