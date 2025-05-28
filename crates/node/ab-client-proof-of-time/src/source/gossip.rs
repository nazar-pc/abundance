//! PoT gossip functionality.

use crate::PotNextSlotInput;
use ab_core_primitives::pot::{PotCheckpoints, PotSeed, SlotNumber};
use parity_scale_codec::{Decode, Encode};
use std::num::NonZeroU32;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Encode, Decode)]
pub struct GossipProof {
    /// Slot number
    pub slot: SlotNumber,
    /// Proof of time seed
    pub seed: PotSeed,
    /// Iterations per slot
    pub slot_iterations: NonZeroU32,
    /// Proof of time checkpoints
    pub checkpoints: PotCheckpoints,
}

#[derive(Debug)]
pub enum ToGossipMessage {
    Proof(GossipProof),
    NextSlotInput(PotNextSlotInput),
}
