//! `#[env]` argument must be named `env` (or `_env` if it is unused), like the field of
//! `InternalArgs` that holds it

mod other_name {
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
        pub fn get(#[env] context: &ab_contracts_common::env::Env<'_>) {
            let _context = context;
        }
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
        #[view]
        pub fn get(#[env] _: &ab_contracts_common::env::Env<'_>) {}
    }
}

fn main() {}
