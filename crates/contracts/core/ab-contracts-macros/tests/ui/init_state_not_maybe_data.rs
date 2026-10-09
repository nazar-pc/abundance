//! The last `#[output]` of `#[init]` without a return value is the initial state of the contract,
//! which the host allocates for exactly one `Self`, so it must be `MaybeData<Self>`

mod alias {
    use ab_contracts_macros::contract;
    use ab_io_type::trivial_type::TrivialType;

    pub type Many<T> = [T; 64];

    #[derive(Debug, Copy, Clone, TrivialType)]
    #[repr(C)]
    pub struct Example {
        pub value: u8,
    }

    #[contract]
    impl Example {
        #[init]
        pub fn new(#[output] state: &mut Many<Self>) {
            *state = [Self { value: 1 }; 64];
        }
    }
}

mod variable_elements {
    use ab_contracts_macros::contract;
    use ab_io_type::trivial_type::TrivialType;
    use ab_io_type::variable_elements::VariableElements;

    #[derive(Debug, Copy, Clone, TrivialType)]
    #[repr(C)]
    pub struct Example {
        pub value: u8,
    }

    #[contract]
    impl Example {
        #[init]
        pub fn new(#[output] state: &mut VariableElements<Self>) {
            state.append(&[Self { value: 1 }]);
        }
    }
}

fn main() {}
