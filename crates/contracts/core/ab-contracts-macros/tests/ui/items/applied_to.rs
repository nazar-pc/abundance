//! `#[contract]` is applied to an implementation for a struct, a trait definition or a trait
//! implementation. Generated code is named after the struct or the trait, and there is one
//! implementation of each method, so the struct and the trait must be named by a simple identifier
//! and generics are not supported.

mod not_impl {
    use ab_contracts_macros::contract;

    #[contract]
    pub struct Example {
        pub value: u8,
    }
}

mod generics {
    use ab_contracts_macros::contract;

    pub struct Example<T> {
        pub value: T,
    }

    #[contract]
    impl<T> Example<T> {
        #[view]
        pub fn get(&self) -> u8 {
            0
        }
    }
}

mod generic_self_type {
    use ab_contracts_macros::contract;

    pub struct Example<T> {
        pub value: T,
    }

    #[contract]
    impl Example<u8> {
        #[view]
        pub fn get(&self) -> u8 {
            self.value
        }
    }
}

mod trait_path {
    use ab_contracts_macros::contract;
    use ab_io_type::trivial_type::TrivialType;

    pub mod counter {
        use ab_contracts_macros::contract;

        #[contract]
        pub trait Counter {
            #[view]
            fn get(#[output] count: &mut u8);
        }
    }

    #[derive(Debug, Copy, Clone, TrivialType)]
    #[repr(C)]
    pub struct Example {
        pub value: u8,
    }

    #[contract]
    impl counter::Counter for Example {
        #[view]
        fn get(#[output] count: &mut u8) {
            *count = 0;
        }
    }
}

mod trait_generics {
    use ab_contracts_macros::contract;

    pub trait Counter<T> {
        fn get(count: &mut T);
    }

    pub struct Example {
        pub value: u8,
    }

    #[contract]
    impl Counter<u8> for Example {
        #[view]
        fn get(#[output] count: &mut u8) {
            *count = 0;
        }
    }
}

fn main() {}
