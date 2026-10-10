//! Arguments must be in this order: `#[env]`, `#[tmp]`, `#[slot]`, `#[input]`, `#[output]`. The
//! method is called with arguments in that order, so any other order must be rejected rather than
//! silently mixing up arguments of the same type.

mod slot_before_tmp {
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
        pub fn store(
            #[slot] _slot: &mut ab_io_type::maybe_data::MaybeData<u8>,
            #[tmp] _tmp: &mut ab_io_type::maybe_data::MaybeData<u8>,
        ) {
        }
    }
}

mod slot_before_env {
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
        pub fn read(
            #[slot] _slot: &ab_io_type::maybe_data::MaybeData<u8>,
            #[env] _env: &ab_contracts_common::env::Env<'_>,
        ) {
        }
    }
}

mod tmp_before_env {
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
        pub fn store(
            #[tmp] _tmp: &mut ab_io_type::maybe_data::MaybeData<u8>,
            #[env] _env: &mut ab_contracts_common::env::Env<'_>,
        ) {
        }
    }
}

mod output_before_input {
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
        pub fn copy(
            #[output] _output: &mut ab_io_type::maybe_data::MaybeData<u8>,
            #[input] _input: &u8,
        ) {
        }
    }
}

fn main() {}
