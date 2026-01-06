extern crate alloc;

use crate::b_64_ext::zbs_64_ext::execute_zbs_64_ext;
use ab_riscv_primitives::instruction::b_64_ext::zbs_64_ext::Zbs64ExtInstruction;
use ab_riscv_primitives::registers::{EReg64, ERegisters64, GenericRegisters64};
use alloc::vec;

#[test]
fn test_bset_basic() {
    let mut regs = ERegisters64::default();

    regs.write(EReg64::A0, 0b1000);
    // Set bit 2
    regs.write(EReg64::A1, 2);

    let instructions = vec![Zbs64ExtInstruction::Bset {
        rd: EReg64::A2,
        rs1: EReg64::A0,
        rs2: EReg64::A1,
    }];

    for instruction in instructions {
        execute_zbs_64_ext(&mut regs, instruction);
    }

    assert_eq!(regs.read(EReg64::A2), 0b1100);
}

#[test]
fn test_bseti_basic() {
    let mut regs = ERegisters64::default();

    regs.write(EReg64::A0, 0b1000);

    let instructions = vec![Zbs64ExtInstruction::Bseti {
        rd: EReg64::A2,
        rs1: EReg64::A0,
        shamt: 0,
    }];

    for instruction in instructions {
        execute_zbs_64_ext(&mut regs, instruction);
    }

    assert_eq!(regs.read(EReg64::A2), 0b1001);
}

#[test]
fn test_bset_high_bit() {
    let mut regs = ERegisters64::default();

    regs.write(EReg64::A0, 0);
    // Set bit 63
    regs.write(EReg64::A1, 63);

    let instructions = vec![Zbs64ExtInstruction::Bset {
        rd: EReg64::A2,
        rs1: EReg64::A0,
        rs2: EReg64::A1,
    }];

    for instruction in instructions {
        execute_zbs_64_ext(&mut regs, instruction);
    }

    assert_eq!(regs.read(EReg64::A2), 0x8000_0000_0000_0000u64);
}

#[test]
fn test_bclr_basic() {
    let mut regs = ERegisters64::default();

    regs.write(EReg64::A0, 0b1111);
    // Clear bit 2
    regs.write(EReg64::A1, 2);

    let instructions = vec![Zbs64ExtInstruction::Bclr {
        rd: EReg64::A2,
        rs1: EReg64::A0,
        rs2: EReg64::A1,
    }];

    for instruction in instructions {
        execute_zbs_64_ext(&mut regs, instruction);
    }

    assert_eq!(regs.read(EReg64::A2), 0b1011);
}

#[test]
fn test_bclri_basic() {
    let mut regs = ERegisters64::default();

    regs.write(EReg64::A0, 0xFFFF_FFFF_FFFF_FFFFu64);

    let instructions = vec![Zbs64ExtInstruction::Bclri {
        rd: EReg64::A2,
        rs1: EReg64::A0,
        shamt: 0,
    }];

    for instruction in instructions {
        execute_zbs_64_ext(&mut regs, instruction);
    }

    assert_eq!(regs.read(EReg64::A2), 0xFFFF_FFFF_FFFF_FFFEu64);
}

#[test]
fn test_bclr_high_bit() {
    let mut regs = ERegisters64::default();

    regs.write(EReg64::A0, 0xFFFF_FFFF_FFFF_FFFFu64);
    // Clear bit 63
    regs.write(EReg64::A1, 63);

    let instructions = vec![Zbs64ExtInstruction::Bclr {
        rd: EReg64::A2,
        rs1: EReg64::A0,
        rs2: EReg64::A1,
    }];

    for instruction in instructions {
        execute_zbs_64_ext(&mut regs, instruction);
    }

    assert_eq!(regs.read(EReg64::A2), 0x7FFF_FFFF_FFFF_FFFFu64);
}

#[test]
fn test_binv_basic() {
    let mut regs = ERegisters64::default();

    regs.write(EReg64::A0, 0b1010);
    // Invert bit 1
    regs.write(EReg64::A1, 1);

    let instructions = vec![Zbs64ExtInstruction::Binv {
        rd: EReg64::A2,
        rs1: EReg64::A0,
        rs2: EReg64::A1,
    }];

    for instruction in instructions {
        execute_zbs_64_ext(&mut regs, instruction);
    }

    assert_eq!(regs.read(EReg64::A2), 0b1000);
}

#[test]
fn test_binvi_basic() {
    let mut regs = ERegisters64::default();

    regs.write(EReg64::A0, 0b1010);

    let instructions = vec![Zbs64ExtInstruction::Binvi {
        rd: EReg64::A2,
        rs1: EReg64::A0,
        shamt: 0,
    }];

    for instruction in instructions {
        execute_zbs_64_ext(&mut regs, instruction);
    }

    assert_eq!(regs.read(EReg64::A2), 0b1011);
}

#[test]
fn test_binv_twice() {
    let mut regs = ERegisters64::default();

    regs.write(EReg64::A0, 0b1010);
    regs.write(EReg64::A1, 2);

    let instructions = vec![
        Zbs64ExtInstruction::Binv {
            rd: EReg64::A2,
            rs1: EReg64::A0,
            rs2: EReg64::A1,
        },
        Zbs64ExtInstruction::Binv {
            rd: EReg64::A3,
            rs1: EReg64::A2,
            rs2: EReg64::A1,
        },
    ];

    for instruction in instructions {
        execute_zbs_64_ext(&mut regs, instruction);
    }

    // Inverting twice should give the original value
    assert_eq!(regs.read(EReg64::A3), 0b1010);
}

#[test]
fn test_bext_basic() {
    let mut regs = ERegisters64::default();

    regs.write(EReg64::A0, 0b1010);
    // Extract bit 1
    regs.write(EReg64::A1, 1);

    let instructions = vec![Zbs64ExtInstruction::Bext {
        rd: EReg64::A2,
        rs1: EReg64::A0,
        rs2: EReg64::A1,
    }];

    for instruction in instructions {
        execute_zbs_64_ext(&mut regs, instruction);
    }

    assert_eq!(regs.read(EReg64::A2), 1);
}

#[test]
fn test_bexti_basic() {
    let mut regs = ERegisters64::default();

    regs.write(EReg64::A0, 0b1010);

    let instructions = vec![Zbs64ExtInstruction::Bexti {
        rd: EReg64::A2,
        rs1: EReg64::A0,
        shamt: 2,
    }];

    for instruction in instructions {
        execute_zbs_64_ext(&mut regs, instruction);
    }

    assert_eq!(regs.read(EReg64::A2), 0);
}

#[test]
fn test_bext_high_bit() {
    let mut regs = ERegisters64::default();

    regs.write(EReg64::A0, 0x8000_0000_0000_0000u64);
    // Extract bit 63
    regs.write(EReg64::A1, 63);

    let instructions = vec![Zbs64ExtInstruction::Bext {
        rd: EReg64::A2,
        rs1: EReg64::A0,
        rs2: EReg64::A1,
    }];

    for instruction in instructions {
        execute_zbs_64_ext(&mut regs, instruction);
    }

    assert_eq!(regs.read(EReg64::A2), 1);
}

#[test]
fn test_bext_zero_bit() {
    let mut regs = ERegisters64::default();

    regs.write(EReg64::A0, 0x7FFF_FFFF_FFFF_FFFFu64);
    // Extract bit 63 (which is 0)
    regs.write(EReg64::A1, 63);

    let instructions = vec![Zbs64ExtInstruction::Bext {
        rd: EReg64::A2,
        rs1: EReg64::A0,
        rs2: EReg64::A1,
    }];

    for instruction in instructions {
        execute_zbs_64_ext(&mut regs, instruction);
    }

    assert_eq!(regs.read(EReg64::A2), 0);
}

#[test]
fn test_combination() {
    let mut regs = ERegisters64::default();

    regs.write(EReg64::A0, 0);
    regs.write(EReg64::A1, 5);

    let instructions = vec![
        // Set bit 5
        Zbs64ExtInstruction::Bset {
            rd: EReg64::A2,
            rs1: EReg64::A0,
            rs2: EReg64::A1,
        },
        // Set bit 10
        Zbs64ExtInstruction::Bseti {
            rd: EReg64::A3,
            rs1: EReg64::A2,
            shamt: 10,
        },
        // Extract bit 5
        Zbs64ExtInstruction::Bext {
            rd: EReg64::A4,
            rs1: EReg64::A3,
            rs2: EReg64::A1,
        },
        // Clear bit 5
        Zbs64ExtInstruction::Bclr {
            rd: EReg64::A5,
            rs1: EReg64::A3,
            rs2: EReg64::A1,
        },
    ];

    for instruction in instructions {
        execute_zbs_64_ext(&mut regs, instruction);
    }

    assert_eq!(regs.read(EReg64::A2), 0b10_0000);
    assert_eq!(regs.read(EReg64::A3), 0b100_0010_0000);
    // bit 5 was set
    assert_eq!(regs.read(EReg64::A4), 1);
    // bit 5 cleared
    assert_eq!(regs.read(EReg64::A5), 0b100_0000_0000);
}
