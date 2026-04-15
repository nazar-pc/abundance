use crate::rv64::test_utils::{execute, initialize_state};
use ab_riscv_primitives::prelude::*;

// CZERO.EQZ - rd = (rs2 == 0) ? 0 : rs1

#[test]
fn test_czero_eqz_condition_zero_yields_zero() {
    // rs2 == 0, so rd must be written with 0 regardless of rs1
    let mut state = initialize_state([ZicondInstruction::CzeroEqz {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A0, 0xDEAD_BEEFu64);
    // condition is zero
    state.regs.write(Reg::A1, 0u64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0);
}

#[test]
fn test_czero_eqz_condition_nonzero_yields_rs1() {
    // rs2 != 0, so rd must receive rs1
    let mut state = initialize_state([ZicondInstruction::CzeroEqz {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A0, 0xDEAD_BEEFu64);
    // condition is nonzero
    state.regs.write(Reg::A1, 1u64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xDEAD_BEEF);
}

#[test]
fn test_czero_eqz_condition_max_nonzero_yields_rs1() {
    let mut state = initialize_state([ZicondInstruction::CzeroEqz {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A0, 0x1234_5678u64);
    // all-ones is nonzero
    state.regs.write(Reg::A1, u64::MAX);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0x1234_5678);
}

#[test]
fn test_czero_eqz_rs1_zero_condition_nonzero_yields_zero() {
    // rs1 is already zero; the result is zero either way, but the path is "nonzero condition"
    let mut state = initialize_state([ZicondInstruction::CzeroEqz {
        rd: Reg::A2,
        rs1: Reg::Zero,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A1, 42u64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0);
}

#[test]
fn test_czero_eqz_rd_zero_hardwired() {
    // Writing to x0 must be ignored (hardwired zero)
    let mut state = initialize_state([ZicondInstruction::CzeroEqz {
        rd: Reg::Zero,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A0, 0xFFFF_FFFFu64);
    state.regs.write(Reg::A1, 1u64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::Zero), 0);
}

#[test]
fn test_czero_eqz_rd_rs1_alias_condition_nonzero() {
    // rd and rs1 alias: rs1 must be read before rd is written
    let mut state = initialize_state([ZicondInstruction::CzeroEqz {
        rd: Reg::A0,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A0, 0xABCDu64);
    // condition nonzero -> rd = rs1 (old value)
    state.regs.write(Reg::A1, 1u64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A0), 0xABCD);
}

#[test]
fn test_czero_eqz_rd_rs2_alias_condition_zero() {
    // rd and rs2 alias: condition is zero, so rd gets 0
    let mut state = initialize_state([ZicondInstruction::CzeroEqz {
        rd: Reg::A1,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A0, 0xDEADu64);
    // condition zero -> rd = 0
    state.regs.write(Reg::A1, 0u64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A1), 0);
}

// CZERO.NEZ - rd = (rs2 != 0) ? 0 : rs1

#[test]
fn test_czero_nez_condition_nonzero_yields_zero() {
    // rs2 != 0, so rd must be written with 0
    let mut state = initialize_state([ZicondInstruction::CzeroNez {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A0, 0xDEAD_BEEFu64);
    // condition is nonzero
    state.regs.write(Reg::A1, 1u64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0);
}

#[test]
fn test_czero_nez_condition_zero_yields_rs1() {
    // rs2 == 0, so rd must receive rs1
    let mut state = initialize_state([ZicondInstruction::CzeroNez {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A0, 0xDEAD_BEEFu64);
    // condition is zero
    state.regs.write(Reg::A1, 0u64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xDEAD_BEEF);
}

#[test]
fn test_czero_nez_condition_max_yields_zero() {
    let mut state = initialize_state([ZicondInstruction::CzeroNez {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A0, 0x1234_5678u64);
    // all-ones is nonzero -> rd = 0
    state.regs.write(Reg::A1, u64::MAX);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0);
}

#[test]
fn test_czero_nez_rs1_zero_condition_zero_yields_zero() {
    // rs1 is already zero, condition is zero -> rd = rs1 = 0
    let mut state = initialize_state([ZicondInstruction::CzeroNez {
        rd: Reg::A2,
        rs1: Reg::Zero,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A1, 0u64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0);
}

#[test]
fn test_czero_nez_rd_zero_hardwired() {
    // Writing to x0 must be ignored (hardwired zero)
    let mut state = initialize_state([ZicondInstruction::CzeroNez {
        rd: Reg::Zero,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A0, 0xFFFF_FFFFu64);
    // condition zero -> would pass rs1 through
    state.regs.write(Reg::A1, 0u64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::Zero), 0);
}

#[test]
fn test_czero_nez_rd_rs1_alias_condition_zero() {
    // rd and rs1 alias: condition zero means rd = rs1 (old value)
    let mut state = initialize_state([ZicondInstruction::CzeroNez {
        rd: Reg::A0,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A0, 0xCAFEu64);
    // condition zero -> rd = rs1 (old A0)
    state.regs.write(Reg::A1, 0u64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A0), 0xCAFE);
}

#[test]
fn test_czero_nez_rd_rs2_alias_condition_nonzero() {
    // rd and rs2 alias: rs2 is nonzero so rd = 0
    let mut state = initialize_state([ZicondInstruction::CzeroNez {
        rd: Reg::A1,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A0, 0xBEEFu64);
    // nonzero -> rd = 0
    state.regs.write(Reg::A1, 5u64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A1), 0);
}

// Complementary pair: czero.eqz + czero.nez should partition the input space

#[test]
fn test_czero_pair_adds_to_rs1_when_condition_nonzero() {
    // czero.eqz(rs1, cond) + czero.nez(rs1, cond) == rs1 (for any single cond value)
    //
    // When cond != 0:
    //   czero.eqz -> rs1
    //   czero.nez -> 0
    //   sum       -> rs1
    let rs1_value = 0xABCD_EF01u64;
    let cond_value = 99u64;

    let mut state_eqz = initialize_state([ZicondInstruction::CzeroEqz {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state_eqz.regs.write(Reg::A0, rs1_value);
    state_eqz.regs.write(Reg::A1, cond_value);
    execute(&mut state_eqz).unwrap();

    let mut state_nez = initialize_state([ZicondInstruction::CzeroNez {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state_nez.regs.write(Reg::A0, rs1_value);
    state_nez.regs.write(Reg::A1, cond_value);
    execute(&mut state_nez).unwrap();

    assert_eq!(
        state_eqz.regs.read(Reg::A2) + state_nez.regs.read(Reg::A2),
        rs1_value
    );
}

#[test]
fn test_czero_pair_adds_to_rs1_when_condition_zero() {
    // When cond == 0:
    //   czero.eqz -> 0
    //   czero.nez -> rs1
    //   sum       -> rs1
    let rs1_value = 0x1234_5678u64;

    let mut state_eqz = initialize_state([ZicondInstruction::CzeroEqz {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state_eqz.regs.write(Reg::A0, rs1_value);
    state_eqz.regs.write(Reg::A1, 0u64);
    execute(&mut state_eqz).unwrap();

    let mut state_nez = initialize_state([ZicondInstruction::CzeroNez {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state_nez.regs.write(Reg::A0, rs1_value);
    state_nez.regs.write(Reg::A1, 0u64);
    execute(&mut state_nez).unwrap();

    assert_eq!(
        state_eqz.regs.read(Reg::A2) + state_nez.regs.read(Reg::A2),
        rs1_value
    );
}
