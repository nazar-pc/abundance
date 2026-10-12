use crate::trivial_type::TrivialType;
use crate::variable_elements::VariableElements;
use core::mem::MaybeUninit;

#[test]
fn from_buffer() {
    let buffer = [1_u32, 2, 3, 4];

    {
        let size = 4 * u32::SIZE;
        let elements = VariableElements::<u32>::from_buffer(&buffer, &size).unwrap();
        assert_eq!(elements.capacity(), 4 * u32::SIZE);
        assert_eq!(elements.get_initialized(), &[1, 2, 3, 4]);
    }

    // Size larger than the buffer, elements beyond the buffer would be exposed otherwise
    {
        let size = 4 * u32::SIZE;
        assert!(VariableElements::<u32>::from_buffer(&buffer[..2], &size).is_none());
    }

    // Size is the number of elements rather than bytes
    {
        let size = 4;
        assert!(VariableElements::<u32>::from_buffer(&buffer, &size).is_none());
    }
}

#[test]
fn from_buffer_mut() {
    let mut buffer = [1_u32, 2, 3, 4];

    {
        let mut size = 4 * u32::SIZE;
        let mut elements =
            VariableElements::<u32>::from_buffer_mut(&mut buffer, &mut size).unwrap();
        assert_eq!(elements.capacity(), 4 * u32::SIZE);
        assert_eq!(elements.get_initialized_mut(), &mut [1, 2, 3, 4]);
    }

    // Size larger than the buffer, elements beyond the buffer would be exposed otherwise
    {
        let mut size = 4 * u32::SIZE;
        assert!(VariableElements::<u32>::from_buffer_mut(&mut buffer[..2], &mut size).is_none());
    }

    // Size is the number of elements rather than bytes
    {
        let mut size = 4;
        assert!(VariableElements::<u32>::from_buffer_mut(&mut buffer, &mut size).is_none());
    }
}

#[test]
fn from_uninit() {
    let mut buffer = [MaybeUninit::new(1_u32); 4];

    {
        let mut size = 0;
        let mut elements = VariableElements::<u32>::from_uninit(&mut buffer, &mut size).unwrap();

        assert_eq!(elements.capacity(), 4 * u32::SIZE);
        // SAFETY: The whole buffer is initialized
        let initialized = unsafe { elements.assume_init(4 * u32::SIZE) };
        assert_eq!(initialized.as_deref(), Some([1, 1, 1, 1].as_slice()));
    }

    // Non-zero size, uninitialized elements would be exposed as initialized otherwise
    {
        let mut uninit_buffer = [MaybeUninit::<u32>::uninit(); 4];
        let mut size = 2 * u32::SIZE;
        assert!(VariableElements::<u32>::from_uninit(&mut uninit_buffer, &mut size).is_none());
    }
}

#[test]
fn count() {
    let buffer = [1_u32, 2, 3];
    let size = 3 * u32::SIZE;
    let elements = VariableElements::<u32>::from_buffer(&buffer, &size).unwrap();

    assert_eq!(elements.size(), 3 * u32::SIZE);
    assert_eq!(elements.count(), 3);
}

#[test]
fn append() {
    let mut buffer = [MaybeUninit::uninit(); 4];
    let mut size = 0;
    let mut elements = VariableElements::<u32>::from_uninit(&mut buffer, &mut size).unwrap();

    assert!(elements.append(&[1, 2]));
    assert_eq!(elements.get_initialized(), &[1, 2]);

    // Not enough capacity
    assert!(!elements.append(&[3, 4, 5]));
    assert_eq!(elements.get_initialized(), &[1, 2]);

    assert!(elements.append(&[3, 4]));
    assert_eq!(elements.get_initialized(), &[1, 2, 3, 4]);
}

#[test]
fn copy_from() {
    // Buffers are larger than the instances, so copying too much is caught without Miri too
    let src_buffer = [1_u64, 2, 3, 4, 5, 6, 7, 8];
    let src_size = u64::SIZE;
    let src = VariableElements::<u64>::from_buffer(&src_buffer[..1], &src_size).unwrap();

    let mut dst_buffer = [0_u64; 8];
    let mut dst_size = 2 * u64::SIZE;
    {
        let mut dst =
            VariableElements::<u64>::from_buffer_mut(&mut dst_buffer[..2], &mut dst_size).unwrap();

        assert!(dst.copy_from(&src));
        assert_eq!(dst.get_initialized(), &[1]);
    }

    assert_eq!(dst_size, u64::SIZE);
    assert_eq!(dst_buffer, [1, 0, 0, 0, 0, 0, 0, 0]);
}
