//! Generated code gives containers in arguments a lifetime that only covers the call, so a method
//! can't declare them with `'static` and keep them after the call. A type alias that hides
//! `'static` doesn't change that, and the error points to the alias in the method signature, for
//! every kind of argument.

mod direct {
    use ab_contracts_macros::contract;
    use ab_io_type::trivial_type::TrivialType;
    use ab_io_type::variable_bytes::VariableBytes;

    #[derive(Debug, Copy, Clone, TrivialType)]
    #[repr(C)]
    pub struct Example {
        pub value: u8,
    }

    #[contract]
    impl Example {
        #[view]
        pub fn keep_input(#[input] _input: &VariableBytes<'static, 8>) {}
    }
}

mod alias {
    use ab_contracts_macros::contract;
    use ab_core_primitives::address::Address;
    use ab_io_type::maybe_data::MaybeData;
    use ab_io_type::trivial_type::TrivialType;
    use ab_io_type::variable_bytes::VariableBytes;
    use core::cell::RefCell;
    use core::mem;

    type Bytes = VariableBytes<'static, 8>;
    type Tmp = MaybeData<'static, u64>;
    type Slot = MaybeData<'static, u64>;
    // Ignores its lifetime, so `'_` in `State<'_, Self>` doesn't show that the lifetime is
    // `'static`
    type State<'a, T> = MaybeData<'static, T>;

    thread_local! {
        static KEPT_OUTPUT: RefCell<Option<Bytes>> = const { RefCell::new(None) };
    }

    #[derive(Debug, Copy, Clone, TrivialType)]
    #[repr(C)]
    pub struct Example {
        pub value: u8,
    }

    #[contract]
    impl Example {
        #[init]
        pub fn new(#[output] _state: &mut State<'_, Self>) {}

        #[update]
        pub fn keep_tmp(#[tmp] _tmp: &mut Tmp) {}

        #[update]
        pub fn keep_slot(#[slot] _slot: &mut Slot) {}

        #[update]
        pub fn keep_slot_with_address(#[slot] (_address, _slot): (&Address, &mut Slot)) {}

        #[view]
        pub fn keep_input(#[input] _input: &Bytes) {}

        #[update]
        pub fn keep_output(#[output] output: &mut Bytes) {
            // What `'static` would allow: keeping a container that views memory of the caller
            // after the call
            let buffer = Box::leak(Box::new([0; 8]));
            let size = Box::leak(Box::new(8));
            let replacement = VariableBytes::from_buffer_mut(buffer, size).unwrap();
            let kept = mem::replace(output, replacement);
            KEPT_OUTPUT.with(|kept_output| *kept_output.borrow_mut() = Some(kept));
        }
    }
}

fn main() {}
