// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Slots functionality for Substrate.
//!
//! Some consensus algorithms have a concept of *slots*, which are intervals in
//! time during which certain events can and/or must occur.  This crate
//! provides generic functionality for slots.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod slots;

use ab_core_primitives::pot::SlotNumber;
use futures::future::Either;
use futures::{Future, TryFutureExt};
use futures_timer::Delay;
use sc_consensus::{BlockImport, JustificationSyncLink};
pub use slots::SlotInfo;
use sp_consensus::{Proposal, Proposer, SyncOracle};
use sp_runtime::traits::{Block as BlockT, HashingFor, Header as HeaderT};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// The changes that need to applied to the storage to create the state for a block.
///
/// See [`sp_state_machine::StorageChanges`] for more information.
pub type StorageChanges<Block> = sp_state_machine::StorageChanges<HashingFor<Block>>;

/// A skeleton implementation for `SlotWorker` which tries to claim a slot at
/// its beginning and tries to produce a block if successfully claimed, timing
/// out if block production takes too long.
#[async_trait::async_trait]
pub trait SimpleSlotWorker<B: BlockT> {
    /// A handle to a `BlockImport`.
    type BlockImport: BlockImport<B> + Send + 'static;

    /// A handle to a `SyncOracle`.
    type SyncOracle: SyncOracle;

    /// A handle to a `JustificationSyncLink`, allows hooking into the sync module to control the
    /// justification sync process.
    type JustificationSyncLink: JustificationSyncLink<B>;

    /// The type of future resolving to the proposer.
    type CreateProposer: Future<Output = Result<Self::Proposer, sp_consensus::Error>>
        + Send
        + Unpin
        + 'static;

    /// The type of proposer to use to build blocks.
    type Proposer: Proposer<B> + Send;

    /// Data associated with a slot claim.
    type Claim: Send + Sync + 'static;

    /// A handle to a `BlockImport`.
    fn block_import(&mut self) -> &mut Self::BlockImport;

    /// Tries to claim the given slot, returning an object with claim data if successful.
    async fn claim_slot(&mut self, header: &B::Header, slot: SlotNumber) -> Option<Self::Claim>;

    /// Return the pre digest data to include in a block authored with the given claim.
    fn pre_digest_data(&self, slot: SlotNumber, claim: &Self::Claim)
    -> Vec<sp_runtime::DigestItem>;

    /// Returns a function which produces a `BlockImportParams`.
    async fn block_import_params(
        &self,
        header: B::Header,
        header_hash: &B::Hash,
        body: Vec<B::Extrinsic>,
        storage_changes: StorageChanges<B>,
        public: Self::Claim,
    ) -> Result<sc_consensus::BlockImportParams<B>, sp_consensus::Error>;

    /// Whether to force authoring if offline.
    fn force_authoring(&self) -> bool;

    /// Returns a handle to a `SyncOracle`.
    fn sync_oracle(&mut self) -> &mut Self::SyncOracle;

    /// Returns a handle to a `JustificationSyncLink`.
    fn justification_sync_link(&mut self) -> &mut Self::JustificationSyncLink;

    /// Returns a `Proposer` to author on top of the given block.
    fn proposer(&mut self, block: &B::Header) -> Self::CreateProposer;

    /// Remaining duration for proposing.
    fn proposing_remaining_duration(&self, slot_info: &SlotInfo<B>) -> Duration;

    /// Propose a block by `Proposer`.
    async fn propose(
        &mut self,
        proposer: Self::Proposer,
        claim: &Self::Claim,
        slot_info: SlotInfo<B>,
        end_proposing_at: Instant,
    ) -> Option<Proposal<B, <Self::Proposer as Proposer<B>>::Proof>> {
        let slot = slot_info.slot;

        let inherent_data = Self::create_inherent_data(&slot_info, end_proposing_at).await?;

        let proposing_remaining_duration =
            end_proposing_at.saturating_duration_since(Instant::now());
        let logs = self.pre_digest_data(slot, claim);

        // deadline our production to 98% of the total time left for proposing. As we deadline
        // the proposing below to the same total time left, the 2% margin should be enough for
        // the result to be returned.
        let proposing = proposer
            .propose(
                inherent_data,
                sp_runtime::generic::Digest { logs },
                proposing_remaining_duration.mul_f32(0.98),
                slot_info.block_size_limit,
            )
            .map_err(|e| sp_consensus::Error::ClientImport(e.to_string()));

        let proposal = match futures::future::select(
            proposing,
            Delay::new(proposing_remaining_duration),
        )
        .await
        {
            Either::Left((Ok(p), _)) => p,
            Either::Left((Err(err), _)) => {
                warn!("Proposing failed: {}", err);

                return None;
            }
            Either::Right(_) => {
                info!(
                    "⌛️ Discarding proposal for slot {:?}; block production took too long",
                    slot,
                );

                return None;
            }
        };

        Some(proposal)
    }

    /// Calls `create_inherent_data` and handles errors.
    async fn create_inherent_data(
        slot_info: &SlotInfo<B>,
        end_proposing_at: Instant,
    ) -> Option<sp_inherents::InherentData> {
        let remaining_duration = end_proposing_at.saturating_duration_since(Instant::now());
        let delay = Delay::new(remaining_duration);
        let cid = slot_info.create_inherent_data.create_inherent_data();
        let inherent_data = match futures::future::select(delay, cid).await {
            Either::Right((Ok(data), _)) => data,
            Either::Right((Err(err), _)) => {
                warn!(
                    "Unable to create inherent data for block {:?}: {}",
                    slot_info.chain_head.hash(),
                    err,
                );

                return None;
            }
            Either::Left(_) => {
                warn!(
                    "Creating inherent data took more time than we had left for slot {:?} for block {:?}.",
                    slot_info.slot,
                    slot_info.chain_head.hash(),
                );

                return None;
            }
        };

        Some(inherent_data)
    }

    /// Implements [`SlotWorker::on_slot`].
    async fn on_slot(&mut self, slot_info: SlotInfo<B>) -> Option<()>
    where
        Self: Sync,
    {
        let slot = slot_info.slot;

        let proposing_remaining_duration = self.proposing_remaining_duration(&slot_info);

        let end_proposing_at = if proposing_remaining_duration == Duration::default() {
            debug!(
                "Skipping proposal slot {:?} since there's no time left to propose",
                slot,
            );

            return None;
        } else {
            Instant::now() + proposing_remaining_duration
        };

        if !self.force_authoring() && self.sync_oracle().is_offline() {
            debug!("Skipping proposal slot. Waiting for the network.");

            return None;
        }

        let claim = self.claim_slot(&slot_info.chain_head, slot).await?;

        debug!("Starting authorship at slot: {slot:?}");

        let proposer = match self.proposer(&slot_info.chain_head).await {
            Ok(p) => p,
            Err(err) => {
                warn!("Unable to author block in slot {slot:?}: {err}");

                return None;
            }
        };

        let proposal = self
            .propose(proposer, &claim, slot_info, end_proposing_at)
            .await?;

        let (header, body) = proposal.block.deconstruct();
        let header_num = *header.number();
        let header_hash = header.hash();
        let parent_hash = *header.parent_hash();

        let block_import_params = match self
            .block_import_params(
                header,
                &header_hash,
                body.clone(),
                proposal.storage_changes,
                claim,
            )
            .await
        {
            Ok(bi) => bi,
            Err(err) => {
                warn!("Failed to create block import params: {}", err);

                return None;
            }
        };

        info!(
            "🔖 Pre-sealed block for proposal at {}. Hash now {:?}, previously {:?}.",
            header_num,
            block_import_params.post_hash(),
            header_hash,
        );

        let header = block_import_params.post_header();
        match self.block_import().import_block(block_import_params).await {
            Ok(res) => {
                res.handle_justification(
                    &header.hash(),
                    *header.number(),
                    self.justification_sync_link(),
                );
            }
            Err(err) => {
                warn!("Error with block built on {:?}: {}", parent_hash, err,);
            }
        }

        Some(())
    }
}
