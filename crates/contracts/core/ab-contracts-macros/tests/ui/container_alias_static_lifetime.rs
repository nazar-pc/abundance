//! A type alias that hides `'static` in a container doesn't let a method keep the container after
//! the call: generated code gives it a lifetime that only covers the call regardless of the type

use ab_contracts_macros::contract;
use ab_io_type::maybe_data::MaybeData;
use ab_io_type::trivial_type::TrivialType;
use ab_io_type::variable_bytes::VariableBytes;
use core::cell::RefCell;
use core::mem;

type Bytes = VariableBytes<'static, 8>;
type Tmp = MaybeData<'static, u64>;

thread_local! {
    static KEPT_OUTPUT: RefCell<Option<Bytes>> = const { RefCell::new(None) };
    static KEPT_TMP: RefCell<Option<Tmp>> = const { RefCell::new(None) };
}

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct Example {
    pub value: u8,
}

#[contract]
impl Example {
    #[update]
    pub fn keep_output(#[output] output: &mut Bytes) {
        let buffer = Box::leak(Box::new([0; 8]));
        let size = Box::leak(Box::new(8));
        let replacement = VariableBytes::from_buffer_mut(buffer, size).unwrap();
        let kept = mem::replace(output, replacement);
        KEPT_OUTPUT.with(|kept_output| *kept_output.borrow_mut() = Some(kept));
    }

    #[update]
    pub fn keep_tmp(#[tmp] tmp: &mut Tmp) {
        let data = Box::leak(Box::new(0));
        let size = Box::leak(Box::new(0));
        let replacement = MaybeData::from_mut(data, size).unwrap();
        let kept = mem::replace(tmp, replacement);
        KEPT_TMP.with(|kept_tmp| *kept_tmp.borrow_mut() = Some(kept));
    }
}

fn main() {}
