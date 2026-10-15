//! `IoTypeMut` is not implemented for views over shared memory, they don't give exclusive access to
//! the value

use ab_io_type::IoTypeMut;
use ab_io_type::maybe_data::MaybeData;

fn require<A>(_value: &mut A)
where
    A: IoTypeMut,
{
}

fn main() {
    let value = 5u64;
    require(&mut MaybeData::from_ref(Some(&value)));
}
