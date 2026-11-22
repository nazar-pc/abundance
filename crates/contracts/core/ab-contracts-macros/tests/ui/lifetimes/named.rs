//! The state, `#[tmp]`, `#[slot]` and the last `#[output]` of `#[init]` without a return value
//! (the initial state) are trusted without being parsed, so lifetimes of their references can't be
//! named, which would allow lending them to calls with `Env::lend_output()`. The lifetime of their
//! containers can't be named either, since a method could move the container out and lend it.

mod state {
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
        pub fn lend_state<'a>(&'a mut self, #[env] env: &mut ab_contracts_common::env::Env<'a>) {
            unimplemented!()
        }
    }
}

mod tmp {
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
        pub fn lend_tmp<'a>(
            #[env] env: &mut ab_contracts_common::env::Env<'a>,
            #[tmp] tmp: &'a mut ab_io_type::maybe_data::MaybeData<'_, u8>,
        ) {
            unimplemented!()
        }
    }
}

mod tmp_container {
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
        pub fn lend_tmp<'a>(
            #[env] env: &mut ab_contracts_common::env::Env<'a>,
            #[tmp] tmp: &mut ab_io_type::maybe_data::MaybeData<'a, u8>,
        ) {
            // The body is never compiled, it shows what the signature would allow: lending the
            // container of `tmp` to a call, which could leave contents behind that are not a valid
            // value, while the host stores `#[tmp]` without parsing it
            let replacement = ab_io_type::maybe_data::MaybeData::from_mut(
                Box::leak(Box::new(0)),
                Box::leak(Box::new(0)),
            )
            .unwrap();
            let tmp = Box::leak(Box::new(core::mem::replace(tmp, replacement)));
            let _tmp = env.lend_output(tmp);
        }
    }
}

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
            unimplemented!()
        }
    }
}

mod slot_container {
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
            unimplemented!()
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
            unimplemented!()
        }
    }
}

mod slot_with_address_container {
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
            unimplemented!()
        }
    }
}

mod init_state {
    use ab_contracts_macros::contract;
    #[expect(unused_imports, reason = "Only used by the method, which is rejected")]
    use ab_io_type::maybe_data::MaybeData;
    use ab_io_type::trivial_type::TrivialType;

    #[derive(Debug, Copy, Clone, TrivialType)]
    #[repr(C)]
    pub struct Example {
        pub value: u8,
    }

    #[contract]
    impl Example {
        #[init]
        pub fn new<'a>(
            #[env] env: &mut ab_contracts_common::env::Env<'a>,
            #[output] state: &'a mut MaybeData<'_, Self>,
        ) {
            unimplemented!()
        }
    }
}

mod init_state_container {
    use ab_contracts_macros::contract;
    #[expect(unused_imports, reason = "Only used by the method, which is rejected")]
    use ab_io_type::maybe_data::MaybeData;
    use ab_io_type::trivial_type::TrivialType;

    #[derive(Debug, Copy, Clone, TrivialType)]
    #[repr(C)]
    pub struct Example {
        pub value: u8,
    }

    #[contract]
    impl Example {
        #[init]
        pub fn new<'a>(
            #[env] env: &mut ab_contracts_common::env::Env<'a>,
            #[output] state: &mut MaybeData<'a, Self>,
        ) {
            unimplemented!()
        }
    }
}

fn main() {}
