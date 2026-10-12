use crate::trivial_type::TrivialType;
use crate::variable_elements::VariableElements;

#[test]
fn copy_from() {
    // Buffers are larger than the instances, so copying too much is caught without Miri too
    let src_buffer = [1_u64, 2, 3, 4, 5, 6, 7, 8];
    let src_size = u64::SIZE;
    let src = VariableElements::<u64>::from_buffer(&src_buffer[..1], &src_size);

    let mut dst_buffer = [0_u64; 8];
    let mut dst_size = 2 * u64::SIZE;
    {
        let mut dst = VariableElements::<u64>::from_buffer_mut(&mut dst_buffer[..2], &mut dst_size);

        assert!(dst.copy_from(&src));
        assert_eq!(dst.get_initialized(), &[1]);
    }

    assert_eq!(dst_size, u64::SIZE);
    assert_eq!(dst_buffer, [1, 0, 0, 0, 0, 0, 0, 0]);
}
