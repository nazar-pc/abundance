//! Contracts decode metadata of the state, `#[slot]` and `#[tmp]` types with `MAX_METADATA_DEPTH`
//! as the max nesting depth, so such types can't be nested deeper

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

mod state {
    use super::L32;
    use ab_contracts_macros::contract;
    use ab_io_type::trivial_type::TrivialType;

    #[derive(Debug, Copy, Clone, TrivialType)]
    #[repr(C)]
    pub struct Example {
        // `Example` adds a level, so the state type is nested as deep as `L33`
        pub value: L32,
    }

    #[contract]
    impl Example {
        #[view]
        pub fn get(&self) -> u8 {
            unimplemented!()
        }
    }
}

mod slot {
    use super::L33;
    use ab_contracts_macros::contract;
    use ab_io_type::maybe_data::MaybeData;
    use ab_io_type::trivial_type::TrivialType;

    #[derive(Debug, Copy, Clone, TrivialType)]
    #[repr(C)]
    pub struct Example {
        pub value: u8,
    }

    #[contract]
    impl Example {
        #[update]
        pub fn set(#[slot] _slot: &mut MaybeData<L33>) {}
    }
}

mod tmp {
    use super::L33;
    use ab_contracts_macros::contract;
    use ab_io_type::maybe_data::MaybeData;
    use ab_io_type::trivial_type::TrivialType;

    #[derive(Debug, Copy, Clone, TrivialType)]
    #[repr(C)]
    pub struct Example {
        pub value: u8,
    }

    #[contract]
    impl Example {
        #[update]
        pub fn set(#[tmp] _tmp: &mut MaybeData<L33>) {}
    }
}

/// Types nested exactly `MAX_METADATA_DEPTH` levels deep are allowed
mod max_depth {
    use super::{L31, L32};
    use ab_contracts_macros::contract;
    use ab_io_type::maybe_data::MaybeData;
    use ab_io_type::trivial_type::TrivialType;

    #[derive(Debug, Copy, Clone, TrivialType)]
    #[repr(C)]
    pub struct Example {
        // `Example` adds a level, so the state type is nested as deep as `L32`
        pub value: L31,
    }

    #[contract]
    impl Example {
        #[update]
        pub fn set(#[tmp] _tmp: &mut MaybeData<L32>, #[slot] _slot: &mut MaybeData<L32>) {}
    }
}

fn main() {}
