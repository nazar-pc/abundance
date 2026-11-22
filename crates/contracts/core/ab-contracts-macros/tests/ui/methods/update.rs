//! `#[update]` method gets the state as `&self` or `&mut self`, the state stays with the host,
//! which persists changes to it after the call

mod consumes_self {
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
        pub fn take(self) -> u8 {
            self.value
        }
    }
}

fn main() {}
