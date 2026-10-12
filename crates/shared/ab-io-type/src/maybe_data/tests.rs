use crate::maybe_data::MaybeData;
use crate::trivial_type::TrivialType;
use core::mem::MaybeUninit;

#[test]
fn get_mut_or_init_with() {
    let mut uninit = MaybeUninit::<u64>::uninit();
    let mut size = 0;
    {
        let mut maybe_data = MaybeData::from_uninit(&mut uninit, &mut size);

        assert_eq!(*maybe_data.get_mut_or_init_with(|data| data.write(1)), 1);
        assert_eq!(maybe_data.get(), Some(&1));

        // Already initialized
        assert_eq!(*maybe_data.get_mut_or_init_with(|data| data.write(2)), 1);
        assert_eq!(maybe_data.get(), Some(&1));
    }

    assert_eq!(size, u64::SIZE);
}

#[test]
fn get_mut_or_init_with_other_reference() {
    let mut other = 2_u64;
    let other_ptr = &raw mut other;

    let mut uninit = MaybeUninit::<u64>::uninit();
    let mut size = 0;
    {
        let mut maybe_data = MaybeData::from_uninit(&mut uninit, &mut size);

        // Returns a reference to other data without initializing provided memory. Safe code can
        // return a leaked `Box` instead, this test uses a local variable to avoid the leak.
        let data = maybe_data.get_mut_or_init_with(|_data| {
            // SAFETY: `other` outlives `maybe_data` and is not accessed while this reference exists
            unsafe { &mut *other_ptr }
        });
        assert_eq!(*data, 2);
        assert_eq!(maybe_data.get(), Some(&2));
    }

    assert_eq!(size, u64::SIZE);
}
