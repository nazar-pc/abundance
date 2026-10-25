//! `#[slot]` is trusted without being parsed, so the lifetime of its container can't be named
//! either, since a method could move the container out and lend it to calls with
//! `Env::lend_output()`

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
            #[slot] slot: &mut ab_io_type::maybe_data::MaybeData<'a, u8>,
        ) {
            let replacement = ab_io_type::maybe_data::MaybeData::from_mut(
                Box::leak(Box::new(0)),
                Box::leak(Box::new(0)),
            )
            .unwrap();
            let slot = Box::leak(Box::new(core::mem::replace(slot, replacement)));
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
                &mut ab_io_type::maybe_data::MaybeData<'a, u8>,
            ),
        ) {
            let replacement = ab_io_type::maybe_data::MaybeData::from_mut(
                Box::leak(Box::new(0)),
                Box::leak(Box::new(0)),
            )
            .unwrap();
            let slot = Box::leak(Box::new(core::mem::replace(slot, replacement)));
            let _slot = env.lend_output(slot);
        }
    }
}

fn main() {}
