extern crate alloc;

use crate::rv64::test_utils::{ExtState, execute, initialize_state};
use crate::zkr::ZkrSeedPoll;
use crate::{CsrError, Csrs, ExecutableInstructionCsr, ExecutionError, RegisterFile};
use ab_riscv_primitives::prelude::*;
use alloc::vec;
use alloc::vec::Vec;
use core::assert_matches;

type TestZkr = ZkrInstruction<Reg<u64>>;

/// A CSR index the extension doesn't own, used to check "ignore" (`Ok(false)`) behavior.
const OTHER_CSR: u16 = 0x300;

/// Convenience wrapper defaulting to a genuine read-write access (`will_write = true`), the
/// documented polling idiom (`csrrw rd, seed, x0`). Tests exercising the pure-read restriction use
/// [`prepare_csr_read`][ExecutableInstructionCsr::prepare_csr_read] directly instead.
fn prepare_read(ext_state: &ExtState, csr_index: u16, raw_value: u64, output: &mut u64) -> bool {
    <TestZkr as ExecutableInstructionCsr<ExtState>>::prepare_csr_read(
        ext_state, csr_index, true, raw_value, output,
    )
    .unwrap()
}

fn prepare_write(
    ext_state: &mut ExtState,
    csr_index: u16,
    write_value: u64,
    output: &mut u64,
) -> bool {
    <TestZkr as ExecutableInstructionCsr<ExtState>>::prepare_csr_write(
        ext_state,
        csr_index,
        write_value,
        output,
    )
    .unwrap()
}

#[test]
fn prepare_csr_read_passes_through_seed() {
    let mut output = 0u64;
    let state = initialize_state::<TestZkr, _>([]);

    assert!(prepare_read(
        &state.ext_state,
        SEED_CSR_INDEX,
        0xDEAD_BEEF,
        &mut output
    ));
    assert_eq!(output, 0xDEAD_BEEF);
}

#[test]
fn prepare_csr_read_ignores_other_csrs() {
    let mut output = 0u64;
    let state = initialize_state::<TestZkr, _>([]);

    assert!(!prepare_read(
        &state.ext_state,
        OTHER_CSR,
        0x1234,
        &mut output
    ));
}

/// Per specification, a pure read (`will_write = false`, e.g. `csrrs`/`csrrc` with `rs1 = x0`)
/// against `seed` is illegal and must be rejected, not silently allowed through
#[test]
fn prepare_csr_read_rejects_pure_read() {
    let mut output = 0u64;
    let state = initialize_state::<TestZkr, _>([]);

    let result = <TestZkr as ExecutableInstructionCsr<ExtState>>::prepare_csr_read(
        &state.ext_state,
        SEED_CSR_INDEX,
        false,
        0xDEAD_BEEF,
        &mut output,
    );
    assert_matches!(
        result,
        Err(CsrError::IllegalRead {
            csr_index: SEED_CSR_INDEX
        })
    );
}

/// A pure read against some other, unrelated CSR must still be ignored (`Ok(false)`) rather than
/// rejected - the illegal-pure-read restriction is specific to `seed`
#[test]
fn prepare_csr_read_pure_read_of_other_csr_is_still_ignored() {
    let mut output = 0u64;
    let state = initialize_state::<TestZkr, _>([]);

    let result = <TestZkr as ExecutableInstructionCsr<ExtState>>::prepare_csr_read(
        &state.ext_state,
        OTHER_CSR,
        false,
        0x1234,
        &mut output,
    );
    assert!(!result.unwrap());
}

#[test]
fn prepare_csr_write_ignores_other_csrs() {
    let mut output = 0u64;
    let mut state = initialize_state::<TestZkr, _>([]);

    assert!(!prepare_write(
        &mut state.ext_state,
        OTHER_CSR,
        0x1234,
        &mut output
    ));
    assert_eq!(output, 0);
}

#[test]
fn prepare_csr_write_ignores_write_value_and_encodes_wait() {
    let mut output = 0u64;
    let mut state = initialize_state::<TestZkr, _>([]);
    state
        .ext_state
        .set_seed_poll_sequence(vec![ZkrSeedPoll::Wait]);

    assert!(prepare_write(
        &mut state.ext_state,
        SEED_CSR_INDEX,
        // Per specification, the write value is fully ignored
        0xFFFF_FFFF_FFFF_FFFF,
        &mut output,
    ));
    assert_eq!(output, 0b01u64 << 30);
}

#[test]
fn prepare_csr_write_encodes_bist() {
    let mut output = 0u64;
    let mut state = initialize_state::<TestZkr, _>([]);
    state
        .ext_state
        .set_seed_poll_sequence(vec![ZkrSeedPoll::Bist]);

    prepare_write(&mut state.ext_state, SEED_CSR_INDEX, 0, &mut output);
    assert_eq!(output, 0b00u64 << 30);
}

#[test]
fn prepare_csr_write_encodes_es16_with_entropy_bits() {
    let mut output = 0u64;
    let mut state = initialize_state::<TestZkr, _>([]);
    state
        .ext_state
        .set_seed_poll_sequence(vec![ZkrSeedPoll::Es16(0xBEEF)]);

    prepare_write(&mut state.ext_state, SEED_CSR_INDEX, 0, &mut output);
    assert_eq!(output, (0b10u64 << 30) | 0xBEEF);
    // Reserved (`[29:24]`) and custom (`[23:16]`) bits must be zero
    assert_eq!(output & 0x3FFF_0000, 0);
}

#[test]
fn prepare_csr_write_encodes_dead() {
    let mut output = 0u64;
    let mut state = initialize_state::<TestZkr, _>([]);
    state
        .ext_state
        .set_seed_poll_sequence(vec![ZkrSeedPoll::Dead]);

    prepare_write(&mut state.ext_state, SEED_CSR_INDEX, 0, &mut output);
    assert_eq!(output, 0b11u64 << 30);
}

#[test]
fn prepare_csr_write_advances_through_poll_sequence_and_holds_last() {
    let mut state = initialize_state::<TestZkr, _>([]);
    state.ext_state.set_seed_poll_sequence(vec![
        ZkrSeedPoll::Bist,
        ZkrSeedPoll::Wait,
        ZkrSeedPoll::Es16(0x1234),
    ]);

    let mut outputs = Vec::new();
    for _ in 0..4 {
        let mut output = 0u64;
        prepare_write(&mut state.ext_state, SEED_CSR_INDEX, 0, &mut output);
        outputs.push(output);
    }

    assert_eq!(
        outputs,
        [
            0b00u64 << 30,
            0b01u64 << 30,
            (0b10u64 << 30) | 0x1234,
            // Sequence exhausted: last entry (`Es16(0x1234)`) repeats
            (0b10u64 << 30) | 0x1234,
        ]
    );
}

/// Simulates the documented polling idiom (repeated `csrrw rd, seed, x0`): each read observes
/// whatever the *previous* write polled and stored, and each write polls again for the *next*
/// read - driven entirely through the raw `Csrs::read_csr`/`write_csr` storage plus
/// `ZkrInstruction`'s `prepare_csr_write`, exactly as `Zicsr`'s `execute()` would sequence them.
#[test]
fn seed_csr_polling_idiom_round_trips_through_storage() {
    let mut state = initialize_state::<TestZkr, _>([]);
    state.ext_state.init_csr(SEED_CSR_INDEX, 0);
    state
        .ext_state
        .set_seed_poll_sequence(vec![ZkrSeedPoll::Wait, ZkrSeedPoll::Es16(0x00FF)]);

    // Reset value reads back as `OPST=BIST` (`0`) before any poll has happened
    assert_eq!(state.ext_state.read_csr(SEED_CSR_INDEX).unwrap(), 0);

    // First `csrrw`: reads the reset value, then polls (`Wait`) and stores the result
    let mut write_output = 0u64;
    prepare_write(&mut state.ext_state, SEED_CSR_INDEX, 0, &mut write_output);
    state
        .ext_state
        .write_csr(SEED_CSR_INDEX, write_output)
        .unwrap();
    assert_eq!(
        state.ext_state.read_csr(SEED_CSR_INDEX).unwrap(),
        0b01u64 << 30
    );

    // Second `csrrw`: reads back `WAIT` from the previous write, then polls again (`Es16`)
    let mut read_output = 0u64;
    let raw = state.ext_state.read_csr(SEED_CSR_INDEX).unwrap();
    prepare_read(&state.ext_state, SEED_CSR_INDEX, raw, &mut read_output);
    assert_eq!(read_output, 0b01u64 << 30);

    let mut write_output = 0u64;
    prepare_write(&mut state.ext_state, SEED_CSR_INDEX, 0, &mut write_output);
    state
        .ext_state
        .write_csr(SEED_CSR_INDEX, write_output)
        .unwrap();
    assert_eq!(
        state.ext_state.read_csr(SEED_CSR_INDEX).unwrap(),
        (0b10u64 << 30) | 0x00FF
    );
}

#[test]
fn execute_is_nop_for_empty_variant_set() {
    // `Zkr` defines no instructions, so running an empty program must succeed trivially
    let mut state = initialize_state::<TestZkr, _>([]);
    execute(&mut state).unwrap();
}

// End-to-end tests below exercise `Zicsr`'s real `execute()` code through `TestZkr`
// (`ZkrInstruction`, which inherits `Zicsr`'s variants), confirming `will_write` is computed and
// threaded through correctly for the various `csrr*` variants.

/// Executing an actual `csrrs rd, seed, x0` (pure read, no write) through `Zicsr`'s `execute()`
/// must surface as illegal CSR access, not silently succeed
#[test]
fn csrrs_pure_read_of_seed_is_rejected_end_to_end() {
    let mut state = initialize_state([TestZkr::Csrrs {
        rd: Reg::A0,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
        csr_index: SEED_CSR_INDEX,
    }]);
    state.ext_state.init_csr(SEED_CSR_INDEX, 0);

    let error = execute(&mut state).unwrap_err();
    assert_matches!(
        error,
        ExecutionError::CsrIllegalRead {
            csr_index: SEED_CSR_INDEX
        }
    );
}

/// The same `csrrs rd, seed, x0` idiom but with `rs1 != x0` (so it does write) must succeed and
/// go through the normal poll/encode path
#[test]
fn csrrs_read_write_of_seed_succeeds_end_to_end() {
    let mut state = initialize_state([TestZkr::Csrrs {
        rd: Reg::A0,
        rs1: Reg::A1,
        rs2: Reg::Zero,
        csr_index: SEED_CSR_INDEX,
    }]);
    state.ext_state.init_csr(SEED_CSR_INDEX, 0);
    state
        .ext_state
        .set_seed_poll_sequence(vec![ZkrSeedPoll::Es16(0x00FF)]);

    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0);
    assert_eq!(
        state.ext_state.read_csr(SEED_CSR_INDEX).unwrap(),
        (0b10u64 << 30) | 0x00FF
    );
}

/// `csrrc rd, seed, x0` (pure read via the "clear bits" form) must be rejected the same way
#[test]
fn csrrc_pure_read_of_seed_is_rejected_end_to_end() {
    let mut state = initialize_state([TestZkr::Csrrc {
        rd: Reg::A0,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
        csr_index: SEED_CSR_INDEX,
    }]);
    state.ext_state.init_csr(SEED_CSR_INDEX, 0);

    let error = execute(&mut state).unwrap_err();
    assert_matches!(
        error,
        ExecutionError::CsrIllegalRead {
            csr_index: SEED_CSR_INDEX
        }
    );
}

/// `csrrsi rd, seed, 0` (pure read via the immediate "set bits" form) must be rejected too
#[test]
fn csrrsi_pure_read_of_seed_is_rejected_end_to_end() {
    let mut state = initialize_state([TestZkr::Csrrsi {
        rd: Reg::A0,
        zimm: 0,
        csr_index: SEED_CSR_INDEX,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.ext_state.init_csr(SEED_CSR_INDEX, 0);

    let error = execute(&mut state).unwrap_err();
    assert_matches!(
        error,
        ExecutionError::CsrIllegalRead {
            csr_index: SEED_CSR_INDEX
        }
    );
}

/// `csrrw rd, seed, x0` always performs a write (even with `rs1 = x0`), so it must succeed - this
/// is the documented polling idiom
#[test]
fn csrrw_of_seed_always_succeeds_end_to_end() {
    let mut state = initialize_state([TestZkr::Csrrw {
        rd: Reg::A0,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
        csr_index: SEED_CSR_INDEX,
    }]);
    state.ext_state.init_csr(SEED_CSR_INDEX, 0);
    state
        .ext_state
        .set_seed_poll_sequence(vec![ZkrSeedPoll::Wait]);

    execute(&mut state).unwrap();
    assert_eq!(
        state.ext_state.read_csr(SEED_CSR_INDEX).unwrap(),
        0b01u64 << 30
    );
}
