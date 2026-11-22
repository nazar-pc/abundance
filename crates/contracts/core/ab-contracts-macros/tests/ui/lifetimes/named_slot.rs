//! `#[slot]` is trusted without being parsed, so lifetimes of its references can't be named, which
//! would allow lending the slot to calls with `Env::lend_output()`

mod slot {
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
        pub fn lend_slot<'a>(
            #[env] env: &mut ab_contracts_common::env::Env<'a>,
            #[slot] slot: &'a mut ab_io_type::maybe_data::MaybeData<'_, u8>,
        ) {
            let _slot = env.lend_output(slot);
        }
    }
}

mod slot_with_address {
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
        pub fn lend_slot<'a>(
            #[env] env: &mut ab_contracts_common::env::Env<'a>,
            #[slot] (_address, slot): (
                &ab_core_primitives::address::Address,
                &'a mut ab_io_type::maybe_data::MaybeData<'_, u8>,
            ),
        ) {
            let _slot = env.lend_output(slot);
        }
    }
}

fn main() {}
