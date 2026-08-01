use crate::rv64::test_utils::{execute, initialize_state};
use ab_riscv_primitives::prelude::*;

#[test]
fn test_wrs_nto_is_nop() {
    let mut state = initialize_state([ZawrsInstruction::WrsNto {
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);

    // Should not error and should not hang
    execute(&mut state).unwrap();
}

#[test]
fn test_wrs_sto_is_nop() {
    let mut state = initialize_state([ZawrsInstruction::WrsSto {
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);

    execute(&mut state).unwrap();
}
