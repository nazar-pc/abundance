use crate::rv64::test_utils::{execute, initialize_state};
use ab_riscv_primitives::prelude::*;

#[test]
fn test_fence_i_is_nop() {
    let mut state = initialize_state([ZifenceiInstruction::FenceI {
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);

    // Should not error
    execute(&mut state).unwrap();
}
