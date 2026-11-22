//! Return values are written by the callee into memory allocated for them by the caller according
//! to metadata, which is only possible for `TrivialType`, other types must use `#[output]`

mod variable_bytes {
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
        pub fn get(&self) -> VariableBytes<'static, 0> {
            unimplemented!()
        }
    }
}

fn main() {}
