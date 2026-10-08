//! `#[view]` method only gets read-only access to state, slots and environment, it can't persist
//! changes and must not be able to call `#[update]` methods

mod self_mut {
    use ab_contracts_macros::contract;
    use ab_io_type::trivial_type::TrivialType;

    #[derive(Debug, Copy, Clone, TrivialType)]
    #[repr(C)]
    pub struct Example {
        pub value: u8,
    }

    // Mutable state in a view
    #[contract]
    impl Example {
        #[view]
        pub fn set(&mut self) {
            self.value = 1;
        }
    }
}

mod slot_mut {
    use ab_contracts_macros::contract;
    use ab_io_type::trivial_type::TrivialType;

    #[derive(Debug, Copy, Clone, TrivialType)]
    #[repr(C)]
    pub struct Example {
        pub value: u8,
    }

    // Mutable slot in a view
    #[contract]
    impl Example {
        #[view]
        pub fn set(#[slot] _slot: &mut ab_io_type::maybe_data::MaybeData<u8>) {}
    }
}

mod env_mut {
    use ab_contracts_macros::contract;
    use ab_io_type::trivial_type::TrivialType;

    #[derive(Debug, Copy, Clone, TrivialType)]
    #[repr(C)]
    pub struct Example {
        pub value: u8,
    }

    // Mutable environment in a view
    #[contract]
    impl Example {
        #[view]
        pub fn call_others(#[env] _env: &mut ab_contracts_common::env::Env<'_>) {}
    }
}

fn main() {}
