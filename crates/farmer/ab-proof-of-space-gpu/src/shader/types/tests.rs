use crate::shader::constants::PARAM_BC;
use crate::shader::types::R;

#[test]
fn test_new_with_data_and_split_symmetry() {
    // Assuming PARAM_BC is defined in scope as a const u32 > 0; test uses valid r < PARAM_BC
    // and data fitting in u32::BITS - bits, per safety constraints. Does not test invalid inputs
    // as methods are unsafe and caller must ensure bounds.
    let bits = (PARAM_BC - 1).bit_width();
    let max_data = if bits == u32::BITS {
        0
    } else {
        (1u32 << (u32::BITS - bits)) - 1
    };

    // Test cases: edge values for r and data
    let test_cases = [
        (0, 0),
        (u32::from(PARAM_BC) - 1, 0),
        (0, max_data),
        (u32::from(PARAM_BC) - 1, max_data),
        // Midpoint example; adjust if PARAM_BC is small
        ((u32::from(PARAM_BC) - 1) / 2, max_data / 2),
    ];

    for &(r, data) in &test_cases {
        let instance = unsafe { R::new_with_data(r, data) };
        let (r_back, data_back) = instance.split();
        assert_eq!(r, r_back, "r mismatch for input r={}, data={}", r, data);
        assert_eq!(
            data, data_back,
            "data mismatch for input r={}, data={}",
            r, data
        );
    }

    // Additional check: plain new() should split to (r, 0)
    for r in [0, u32::from(PARAM_BC) / 2, u32::from(PARAM_BC) - 1] {
        let instance = unsafe { R::new(r) };
        let (r_back, data_back) = instance.split();
        assert_eq!(r, r_back, "r mismatch for plain new(r={})", r);
        assert_eq!(0, data_back, "data not zero for plain new(r={})", r);
    }
}
