//! `#[tmp]` and `#[slot]` arguments are references to storage of the contract that the host
//! provides, and `#[slot]` can also be a tuple of a shared reference to the address of the owner of
//! the slot and a reference to the slot. A contract has one `#[tmp]` type and one `#[slot]` type
//! (`Contract::Tmp` and `Contract::Slot`), so all its methods must use the same one.

mod slot_types_differ {
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
        pub fn set(#[slot] _slot: &mut ab_io_type::maybe_data::MaybeData<'_, u8>) {}

        #[view]
        pub fn get(#[slot] _slot: &ab_io_type::maybe_data::MaybeData<'_, u16>) {}
    }
}

fn main() {}
