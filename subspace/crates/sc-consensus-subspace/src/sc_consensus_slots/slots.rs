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

//! Utility stream for yielding slots in a loop.
//!
//! This is used instead of `futures_timer::Interval` because it was unreliable.

use ab_core_primitives::pot::SlotNumber;
use sp_inherents::InherentDataProvider;
use sp_runtime::traits::Block as BlockT;
use std::time::Duration;

/// Information about a slot.
pub struct SlotInfo<B: BlockT> {
    /// The slot number as found in the inherent data.
    pub slot: SlotNumber,
    /// The inherent data provider.
    pub create_inherent_data: Box<dyn InherentDataProvider>,
    /// Slot duration.
    pub duration: Duration,
    /// The chain header this slot is based on.
    pub chain_head: B::Header,
    /// Some potential block size limit for the block to be authored at this slot.
    ///
    /// For more information see [`Proposer::propose`](sp_consensus::Proposer::propose).
    pub block_size_limit: Option<usize>,
}

impl<B: BlockT> SlotInfo<B> {
    /// Create a new [`SlotInfo`].
    ///
    /// `ends_at` is calculated using `timestamp` and `duration`.
    pub fn new(
        slot: SlotNumber,
        create_inherent_data: Box<dyn InherentDataProvider>,
        duration: Duration,
        chain_head: B::Header,
        block_size_limit: Option<usize>,
    ) -> Self {
        Self {
            slot,
            create_inherent_data,
            duration,
            chain_head,
            block_size_limit,
        }
    }
}
