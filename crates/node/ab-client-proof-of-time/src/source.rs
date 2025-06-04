pub mod block_import;
pub mod gossip;
pub mod state;
pub mod timekeeper;

use crate::PotNextSlotInput;
use crate::source::block_import::BestBlockPotInfo;
use crate::source::gossip::{GossipProof, ToGossipMessage};
use crate::source::state::{PotState, PotStateSetOutcome};
use crate::source::timekeeper::TimekeeperProof;
use ab_client_api::ChainInfo;
use ab_core_primitives::pot::{PotCheckpoints, SlotNumber};
use derive_more::{Deref, DerefMut};
use futures::channel::mpsc;
use futures::{FutureExt, StreamExt, select};
use std::future;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, trace, warn};

const SLOTS_CHANNEL_CAPACITY: usize = 10;

/// Proof of time slot information
#[derive(Debug, Copy, Clone)]
pub struct PotSlotInfo {
    /// Slot number
    pub slot: SlotNumber,
    /// Proof of time checkpoints
    pub checkpoints: PotCheckpoints,
}

/// Stream with proof of time slots
#[derive(Debug, Deref, DerefMut)]
pub struct PotSlotInfoStream(broadcast::Receiver<PotSlotInfo>);

impl Clone for PotSlotInfoStream {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.resubscribe())
    }
}

/// Worker producing proofs of time.
///
/// Depending on configuration may produce proofs of time locally, send/receive via gossip and keep
/// up to day with blockchain reorgs.
#[derive(Debug)]
#[must_use = "Proof of time source doesn't do anything unless run() method is called"]
pub struct PotSourceWorker<CS> {
    chain_info: CS,
    timekeeper_proof_receiver: Option<mpsc::Receiver<TimekeeperProof>>,
    to_gossip_sender: mpsc::Sender<ToGossipMessage>,
    from_gossip_receiver: mpsc::Receiver<GossipProof>,
    best_block_pot_info_receiver: mpsc::Receiver<BestBlockPotInfo>,
    last_slot_sent: SlotNumber,
    slot_sender: broadcast::Sender<PotSlotInfo>,
    pot_state: Arc<PotState>,
}

impl<CI> PotSourceWorker<CI>
where
    CI: ChainInfo,
{
    pub fn new(
        timekeeper_proof_receiver: Option<mpsc::Receiver<TimekeeperProof>>,
        to_gossip_sender: mpsc::Sender<ToGossipMessage>,
        from_gossip_receiver: mpsc::Receiver<GossipProof>,
        best_block_pot_info_receiver: mpsc::Receiver<BestBlockPotInfo>,
        chain_info: CI,
        pot_state: Arc<PotState>,
    ) -> (Self, PotSlotInfoStream) {
        let (slot_sender, slot_receiver) = broadcast::channel(SLOTS_CHANNEL_CAPACITY);

        let source_worker = Self {
            chain_info,
            timekeeper_proof_receiver,
            to_gossip_sender,
            from_gossip_receiver,
            best_block_pot_info_receiver,
            last_slot_sent: SlotNumber::ZERO,
            slot_sender,
            pot_state,
        };

        let pot_slot_info_stream = PotSlotInfoStream(slot_receiver);

        (source_worker, pot_slot_info_stream)
    }

    /// Run proof of time source
    pub async fn run(mut self) {
        loop {
            let timekeeper_proof = async {
                if let Some(timekeeper_proof_receiver) = &mut self.timekeeper_proof_receiver {
                    timekeeper_proof_receiver.next().await
                } else {
                    future::pending().await
                }
            };

            select! {
                maybe_timekeeper_proof = timekeeper_proof.fuse() => {
                    if let Some(timekeeper_proof) = maybe_timekeeper_proof {
                        self.handle_timekeeper_proof(timekeeper_proof);
                    } else {
                        debug!("Timekeeper proof stream ended, exiting");
                        return;
                    }
                }
                maybe_gossip_proof = self.from_gossip_receiver.next() => {
                    if let Some(gossip_proof) = maybe_gossip_proof {
                        self.handle_gossip_proof(gossip_proof);
                    } else {
                        debug!("Incoming gossip messages stream ended, exiting");
                        return;
                    }
                }
                maybe_best_block_pot_info = self.best_block_pot_info_receiver.next() => {
                    if let Some(best_block_pot_info) = maybe_best_block_pot_info {
                        self.handle_best_block_pot_info(best_block_pot_info);
                    } else {
                        debug!("Import notifications stream ended, exiting");
                        return;
                    }
                }
            }
        }
    }

    fn handle_timekeeper_proof(&mut self, proof: TimekeeperProof) {
        let TimekeeperProof {
            slot,
            seed,
            slot_iterations,
            checkpoints,
        } = proof;

        if self.chain_info.is_syncing() {
            trace!(
                ?slot,
                %seed,
                %slot_iterations,
                output = %checkpoints.output(),
                "Ignore timekeeper proof due to major syncing",
            );

            return;
        }

        debug!(
            ?slot,
            %seed,
            %slot_iterations,
            output = %checkpoints.output(),
            "Received timekeeper proof",
        );

        if self
            .to_gossip_sender
            .try_send(ToGossipMessage::Proof(GossipProof {
                slot,
                seed,
                slot_iterations,
                checkpoints,
            }))
            .is_err()
        {
            debug!(
                %slot,
                "Gossip is not able to keep-up with slot production (timekeeper)",
            );
        }

        if slot > self.last_slot_sent {
            self.last_slot_sent = slot;

            // We don't care if block production is too slow or block production is not enabled on this
            // node at all
            let _ = self.slot_sender.send(PotSlotInfo { slot, checkpoints });
        }
    }

    // TODO: Follow both verified and unverified checkpoints to start secondary timekeeper ASAP in
    //  case verification succeeds
    fn handle_gossip_proof(&mut self, proof: GossipProof) {
        let expected_next_slot_input = PotNextSlotInput {
            slot: proof.slot,
            slot_iterations: proof.slot_iterations,
            seed: proof.seed,
        };

        if let Ok(next_slot_input) = self.pot_state.try_extend(
            expected_next_slot_input,
            proof.slot,
            proof.checkpoints.output(),
            None,
        ) {
            if proof.slot > self.last_slot_sent {
                self.last_slot_sent = proof.slot;

                // We don't care if block production is too slow or block production is not enabled on
                // this node at all
                let _ = self.slot_sender.send(PotSlotInfo {
                    slot: proof.slot,
                    checkpoints: proof.checkpoints,
                });
            }

            if self
                .to_gossip_sender
                .try_send(ToGossipMessage::NextSlotInput(next_slot_input))
                .is_err()
            {
                debug!(
                    slot = %proof.slot,
                    next_slot = %next_slot_input.slot,
                    "Gossip is not able to keep-up with slot production (gossip)",
                );
            }
        }
    }

    fn handle_best_block_pot_info(&mut self, best_block_pot_info: BestBlockPotInfo) {
        // This will do one of 3 things depending on circumstances:
        // * if block import is ahead of timekeeper and gossip, it will update next slot input
        // * if block import is on a different PoT chain, it will update next slot input to the
        //   correct fork (reorg)
        // * if block import is on the same PoT chain this will essentially do nothing
        match self.pot_state.set_known_good_output(
            best_block_pot_info.slot,
            best_block_pot_info.pot_output,
            best_block_pot_info.pot_parameters_change,
        ) {
            PotStateSetOutcome::NoChange => {
                trace!(
                    slot = %best_block_pot_info.slot,
                    "Block import didn't result in proof of time chain changes",
                );
            }
            PotStateSetOutcome::Extension { from, to } => {
                warn!(
                    from_next_slot = %from.slot,
                    to_next_slot = %to.slot,
                    "Proof of time chain was extended from block import",
                );

                if self
                    .to_gossip_sender
                    .try_send(ToGossipMessage::NextSlotInput(to))
                    .is_err()
                {
                    debug!(
                        next_slot = %to.slot,
                        "Gossip is not able to keep-up with slot production (block import)",
                    );
                }
            }
            PotStateSetOutcome::Reorg { from, to } => {
                warn!(
                    from_next_slot = %from.slot,
                    to_next_slot = %to.slot,
                    "Proof of time chain reorg happened",
                );

                if self
                    .to_gossip_sender
                    .try_send(ToGossipMessage::NextSlotInput(to))
                    .is_err()
                {
                    debug!(
                        next_slot = %to.slot,
                        "Gossip is not able to keep-up with slot production (block import)",
                    );
                }
            }
        }
    }
}
