//! `#[contract]` is applied to an implementation for a struct, a trait definition or a trait
//! implementation. Generated code is named after the struct or the trait, and there is one
//! implementation of each method, so the struct must be named by a simple identifier and generics
//! are not supported.

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

fn main() {}
