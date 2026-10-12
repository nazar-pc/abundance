//! Contracts decode metadata of method arguments and return types with `MAX_METADATA_DEPTH` as
//! the max nesting depth, so such types can't be nested deeper

use ab_io_type::trivial_type::TrivialType;

/// Declares tuple structs nested in each other, the first one contains `$inner`
macro_rules! nested_structs {
    ($inner:ty, $name:ident $(, $rest:ident )*) => {
        #[derive(Debug, Copy, Clone, TrivialType)]
        #[repr(C)]
        pub struct $name($inner);

        nested_structs!($name $(, $rest )*);
    };
    ($inner:ty) => {};
}

nested_structs!(
    u8, L1, L2, L3, L4, L5, L6, L7, L8, L9, L10, L11, L12, L13, L14, L15, L16, L17, L18, L19, L20,
    L21, L22, L23, L24, L25, L26, L27, L28, L29, L30, L31, L32, L33
);

mod input {
    use super::L33;
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
        pub fn get(#[input] _value: &L33) {}
    }
}

mod output {
    use super::L33;
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
        pub fn get(#[output] _value: &mut L33) {}
    }
}

mod return_type {
    use super::L33;
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
        pub fn get() -> L33 {
            unimplemented!()
        }
    }
}

mod trait_definition {
    use super::L33;
    use ab_contracts_common::env::Env;
    use ab_contracts_macros::contract;

    #[contract]
    pub trait Getter {
        #[view]
        fn get(#[env] env: &Env<'_>, #[input] value: &L33);
    }
}

mod trait_implementation {
    use super::L33;
    use super::trait_definition::Getter;
    use ab_contracts_common::env::Env;
    use ab_contracts_macros::contract;
    use ab_io_type::trivial_type::TrivialType;

    #[derive(Debug, Copy, Clone, TrivialType)]
    #[repr(C)]
    pub struct Example {
        pub value: u8,
    }

    #[contract]
    impl Example {}

    #[contract]
    impl Getter for Example {
        #[view]
        fn get(#[env] _env: &Env<'_>, #[input] _value: &L33) {}
    }
}

/// Types nested exactly `MAX_METADATA_DEPTH` levels deep are allowed
mod max_depth {
    use super::L32;
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
        pub fn get(#[input] _input: &L32, #[output] _output: &mut L32) -> L32 {
            unimplemented!()
        }
    }
}

fn main() {}
