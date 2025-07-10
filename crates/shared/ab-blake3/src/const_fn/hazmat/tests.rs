use crate::const_fn::CHUNK_LEN;
use crate::const_fn::hazmat::left_subtree_len;

#[test]
fn test_left_subtree_len() {
    assert_eq!(left_subtree_len(1025), 1024);
    for boundary_case in [2, 4, 8, 16, 32, 64] {
        let input_len = boundary_case * CHUNK_LEN as u64;
        assert_eq!(left_subtree_len(input_len - 1), input_len / 2);
        assert_eq!(left_subtree_len(input_len), input_len / 2);
        assert_eq!(left_subtree_len(input_len + 1), input_len);
    }
}
