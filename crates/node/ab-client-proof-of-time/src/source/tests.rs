use crate::PotNextSlotInput;
use crate::source::block_import::BestBlockPotInfo;
use crate::source::gossip::ToGossipMessage;
use crate::source::state::PotState;
use crate::source::timekeeper::TimekeeperProof;
use crate::source::{PotSlotInfoStream, PotSourceWorker};
use crate::test_utils::{GENESIS_SEED, SLOT_ITERATIONS, first_slot_input, prove_slots};
use crate::verifier::PotVerifier;
use ab_client_api::ChainSyncStatus;
use ab_core_primitives::block::BlockNumber;
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pot::{PotCheckpoints, PotParametersChange, PotSeed, SlotNumber};
use futures::channel::mpsc;
use std::assert_matches;
use tokio::sync::broadcast;
use triomphe::Arc;

#[derive(Debug, Clone)]
struct NotSyncing;

impl ChainSyncStatus for NotSyncing {
    fn target_block_number(&self) -> BlockNumber {
        BlockNumber::ZERO
    }

    fn is_syncing(&self) -> bool {
        false
    }

    fn is_offline(&self) -> bool {
        false
    }
}

struct TestSetup {
    verifier: PotVerifier,
    pot_state: Arc<PotState>,
    worker: PotSourceWorker<NotSyncing>,
    slot_info_stream: PotSlotInfoStream,
    to_gossip_receiver: mpsc::Receiver<ToGossipMessage>,
    /// Slots 1..=2
    chain: Vec<(PotNextSlotInput, PotCheckpoints)>,
    /// Slots 2..=3, with entropy injected at slot 2
    fork: Vec<(PotNextSlotInput, PotCheckpoints)>,
    fork_parameters_change: PotParametersChange,
}

impl TestSetup {
    fn new() -> Self {
        let verifier = PotVerifier::new(PotSeed::from(GENESIS_SEED), 100);
        let pot_state = Arc::new(PotState::new(first_slot_input(), None, verifier.clone()));
        let (to_gossip_sender, to_gossip_receiver) = mpsc::channel(10);
        let (_from_gossip_sender, from_gossip_receiver) = mpsc::channel(10);
        let (_best_block_pot_info_sender, best_block_pot_info_receiver) = mpsc::channel(10);
        let (worker, slot_info_stream) = PotSourceWorker::new(
            None,
            to_gossip_sender,
            from_gossip_receiver,
            best_block_pot_info_receiver,
            NotSyncing,
            Arc::clone(&pot_state),
        );

        let chain = prove_slots(first_slot_input(), 2);
        let fork_parameters_change = PotParametersChange {
            slot: SlotNumber::from(2),
            slot_iterations: SLOT_ITERATIONS,
            entropy: Blake3Hash::new([1; _]),
        };
        let fork = prove_slots(
            PotNextSlotInput::derive(
                SLOT_ITERATIONS,
                SlotNumber::ONE,
                chain[0].1.output(),
                &Some(fork_parameters_change),
            ),
            2,
        );

        Self {
            verifier,
            pot_state,
            worker,
            slot_info_stream,
            to_gossip_receiver,
            chain,
            fork,
            fork_parameters_change,
        }
    }

    /// Prove the slot the way timekeeper does and process the proof by the worker afterward
    fn timekeeper_proof(&mut self, (slot_input, checkpoints): (PotNextSlotInput, PotCheckpoints)) {
        self.verifier.inject_verified_checkpoints(
            slot_input.seed,
            slot_input.slot_iterations,
            checkpoints,
        );
        let _: Result<_, _> =
            self.pot_state
                .try_extend(slot_input, slot_input.slot, checkpoints.output(), None);
        self.worker.handle_timekeeper_proof(TimekeeperProof {
            slot: slot_input.slot,
            seed: slot_input.seed,
            slot_iterations: slot_input.slot_iterations,
            checkpoints,
        });
    }

    /// Import a best block of the fork with the future proof of time at slot 2
    fn import_fork_block(&mut self) {
        self.worker.handle_best_block_pot_info(BestBlockPotInfo {
            slot: SlotNumber::from(2),
            pot_output: self.fork[0].1.output(),
            pot_parameters_change: Some(self.fork_parameters_change),
        });
        assert_matches!(
            self.to_gossip_receiver.try_recv(),
            Ok(ToGossipMessage::NextSlotInput(_))
        );
    }

    fn assert_slot_sent(&mut self, slot: SlotNumber) {
        assert_eq!(self.slot_info_stream.try_recv().unwrap().slot, slot);
        assert_matches!(
            self.to_gossip_receiver.try_recv(),
            Ok(ToGossipMessage::Proof(proof)) if proof.slot == slot
        );
    }

    fn assert_nothing_sent(&mut self) {
        assert_matches!(
            self.slot_info_stream.try_recv(),
            Err(broadcast::error::TryRecvError::Empty)
        );
        assert_matches!(
            self.to_gossip_receiver.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        );
    }
}

#[test]
fn stale_timekeeper_proofs_are_ignored() {
    let mut setup = TestSetup::new();

    setup.timekeeper_proof(setup.chain[0]);
    setup.assert_slot_sent(SlotNumber::ONE);

    // Timekeeper proves slot 2, but the fork is imported before the proof is processed
    let (slot_input, checkpoints) = setup.chain[1];
    setup.verifier.inject_verified_checkpoints(
        slot_input.seed,
        slot_input.slot_iterations,
        checkpoints,
    );
    setup
        .pot_state
        .try_extend(slot_input, slot_input.slot, checkpoints.output(), None)
        .unwrap();
    setup.import_fork_block();
    setup.worker.handle_timekeeper_proof(TimekeeperProof {
        slot: slot_input.slot,
        seed: slot_input.seed,
        slot_iterations: slot_input.slot_iterations,
        checkpoints,
    });
    setup.assert_nothing_sent();

    setup.timekeeper_proof(setup.fork[1]);
    setup.assert_slot_sent(SlotNumber::from(3));
}
