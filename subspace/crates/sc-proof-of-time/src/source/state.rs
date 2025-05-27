use crate::verifier::PotVerifier;
use ab_core_primitives::pot::{PotOutput, PotParametersChange, SlotNumber};
use parking_lot::Mutex;
use sp_consensus_subspace::PotNextSlotInput;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
struct InnerState {
    next_slot_input: PotNextSlotInput,
    parameters_change: Option<PotParametersChange>,
}

impl InnerState {
    pub(super) fn update(
        mut self,
        mut best_slot: SlotNumber,
        mut best_output: PotOutput,
        maybe_updated_parameters_change: Option<Option<PotParametersChange>>,
        pot_verifier: &PotVerifier,
    ) -> Self {
        if let Some(updated_parameters_change) = maybe_updated_parameters_change {
            self.parameters_change = updated_parameters_change;
        }

        loop {
            self.next_slot_input = PotNextSlotInput::derive(
                self.next_slot_input.slot_iterations,
                best_slot,
                best_output,
                &self.parameters_change,
            );

            // Advance further as far as possible using previously verified proofs/checkpoints
            if let Some(checkpoints) = pot_verifier.try_get_checkpoints(
                self.next_slot_input.slot_iterations,
                self.next_slot_input.seed,
            ) {
                best_slot = self.next_slot_input.slot;
                best_output = checkpoints.output();
            } else {
                break;
            }
        }

        self
    }
}

#[derive(Debug)]
pub(super) enum PotStateUpdateOutcome {
    NoChange,
    Extension {
        from: PotNextSlotInput,
        to: PotNextSlotInput,
    },
    Reorg {
        from: PotNextSlotInput,
        to: PotNextSlotInput,
    },
}

/// Global PoT state.
///
/// Maintains the accurate information about PoT state and current tip.
#[derive(Debug)]
pub(super) struct PotState {
    inner_state: Mutex<InnerState>,
    verifier: PotVerifier,
}

impl PotState {
    /// Create a new PoT state
    pub(super) fn new(
        next_slot_input: PotNextSlotInput,
        parameters_change: Option<PotParametersChange>,
        verifier: PotVerifier,
    ) -> Self {
        let inner = InnerState {
            next_slot_input,
            parameters_change,
        };

        Self {
            inner_state: Mutex::new(inner),
            verifier,
        }
    }

    /// PoT input for the next slot
    pub(super) fn next_slot_input(&self) -> PotNextSlotInput {
        self.inner_state.lock().next_slot_input
    }

    /// Extend PoT chain if it matches provided expected next slot input.
    ///
    /// Returns `Ok(new_next_slot_input)` if PoT chain was extended successfully and
    /// `Err(existing_next_slot_input)` in case the state was changed in the meantime.
    pub(super) fn try_extend(
        &self,
        expected_existing_next_slot_input: PotNextSlotInput,
        best_slot: SlotNumber,
        best_output: PotOutput,
        maybe_updated_parameters_change: Option<Option<PotParametersChange>>,
    ) -> Result<PotNextSlotInput, PotNextSlotInput> {
        let mut existing_inner_state = self.inner_state.lock();
        if expected_existing_next_slot_input != existing_inner_state.next_slot_input {
            return Err(existing_inner_state.next_slot_input);
        }

        *existing_inner_state = existing_inner_state.update(
            best_slot,
            best_output,
            maybe_updated_parameters_change,
            &self.verifier,
        );

        Ok(existing_inner_state.next_slot_input)
    }

    /// Set known goo output for time slot, overriding PoT chain if it doesn't match the provided
    /// output.
    ///
    /// This is typically called with information obtained from received block. It typically lags
    /// behind PoT tip and is used as a correction mechanism in case PoT reorg is needed.
    pub(super) fn set_known_good_output(
        &self,
        slot: SlotNumber,
        output: PotOutput,
        maybe_updated_parameters_change: Option<Option<PotParametersChange>>,
    ) -> PotStateUpdateOutcome {
        let previous_best_state;
        let new_best_state;
        {
            let mut inner_state = self.inner_state.lock();
            previous_best_state = *inner_state;
            new_best_state = previous_best_state.update(
                slot,
                output,
                maybe_updated_parameters_change,
                &self.verifier,
            );
            *inner_state = new_best_state;
        }

        if previous_best_state.next_slot_input == new_best_state.next_slot_input {
            return PotStateUpdateOutcome::NoChange;
        }

        if previous_best_state.next_slot_input.slot < new_best_state.next_slot_input.slot {
            let mut slot_iterations = previous_best_state.next_slot_input.slot_iterations;
            let mut seed = previous_best_state.next_slot_input.seed;

            for slot in
                previous_best_state.next_slot_input.slot..new_best_state.next_slot_input.slot
            {
                let Some(checkpoints) = self.verifier.try_get_checkpoints(slot_iterations, seed)
                else {
                    break;
                };

                let pot_input = PotNextSlotInput::derive(
                    slot_iterations,
                    slot,
                    checkpoints.output(),
                    &maybe_updated_parameters_change.flatten(),
                );

                // TODO: Consider carrying of the whole `PotNextSlotInput` rather than individual
                //  variables
                let next_slot = slot + SlotNumber::ONE;
                slot_iterations = pot_input.slot_iterations;
                seed = pot_input.seed;

                if next_slot == new_best_state.next_slot_input.slot
                    && slot_iterations == new_best_state.next_slot_input.slot_iterations
                    && seed == new_best_state.next_slot_input.seed
                {
                    return PotStateUpdateOutcome::Extension {
                        from: previous_best_state.next_slot_input,
                        to: new_best_state.next_slot_input,
                    };
                }
            }
        }

        PotStateUpdateOutcome::Reorg {
            from: previous_best_state.next_slot_input,
            to: new_best_state.next_slot_input,
        }
    }
}
