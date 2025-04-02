#![feature(vec_deque_pop_if)]

use ab_contracts_common::block::{BlockHash, BlockNumber};
use ab_transaction::TransactionHash;
use ab_transaction::owned::OwnedTransaction;
use std::collections::{HashMap, HashSet, VecDeque};
use std::num::{NonZeroU8, NonZeroU64, NonZeroUsize};

/// Transaction pool limits
#[derive(Debug, Copy, Clone)]
pub struct TransactionPoolLimits {
    /// Number of transactions
    pub count: NonZeroUsize,
    /// Total size of all transactions
    pub size: NonZeroUsize,
}

#[derive(Debug)]
pub struct TransactionAuthorizedDetails {
    /// Block number at which transaction was authorized
    pub block_number: BlockNumber,
    /// Block hash at which transaction was authorized
    pub block_hash: BlockHash,
}

#[derive(Debug)]
pub enum TransactionState {
    New,
    Authorized {
        at: VecDeque<TransactionAuthorizedDetails>,
    },
}

#[derive(Debug)]
#[non_exhaustive]
pub struct PoolTransaction {
    pub tx: OwnedTransaction,
    pub state: TransactionState,
    // TODO: Slots, other things?
}

/// Error for [`TransactionPool::add()`] method
#[derive(Debug, thiserror::Error)]
pub enum TransactionAddError {
    /// Already exists
    #[error("Already exists")]
    AlreadyExists,
    /// The block isn't found, possibly too old
    #[error("The block isn't found, possibly too old")]
    BlockNotFound,
    /// Too many transactions
    #[error("Too many transactions")]
    TooManyTransactions,
    /// Total size too large
    #[error("Total size too large")]
    TotalSizeTooLarge,
}

#[derive(Debug)]
struct BlockHashDetails {
    txs: HashSet<TransactionHash>,
}

// TODO: Some integration or tracking of slots, including optimization where only changes to read
//  slots impact transaction authorization
/// Transaction pool implementation.
///
/// The goal of the transaction pool is to retain a set of transactions and associated authorization
/// information, which can be used for block production and propagation through the network.
#[derive(Debug)]
pub struct TransactionPool {
    transactions: HashMap<TransactionHash, PoolTransaction>,
    total_size: usize,
    /// Map from block hash at which transactions were created to a set of transaction hashes
    by_block_hash: HashMap<BlockHash, BlockHashDetails>,
    // TODO: Optimize with an oldest block + `Vec<BlockHash>` instead
    by_block_number: HashMap<BlockNumber, BlockHash>,
    pruning_depth: NonZeroU64,
    authorization_history_depth: NonZeroU8,
    limits: TransactionPoolLimits,
}

impl TransactionPool {
    /// Create new instance.
    ///
    /// `pruning_depth` defines how old (in blocks) should transaction be before it is automatically
    /// removed from the transaction pool.
    ///
    /// `authorization_history_depth` defines a small number of recent blocks for which
    /// authorization information is retained in each block.
    ///
    /// `limits` defines the limits of transaction pool.
    pub fn new(
        pruning_depth: NonZeroU64,
        authorization_history_depth: NonZeroU8,
        limits: TransactionPoolLimits,
    ) -> Self {
        Self {
            transactions: HashMap::default(),
            total_size: 0,
            by_block_hash: HashMap::default(),
            by_block_number: HashMap::default(),
            pruning_depth,
            authorization_history_depth,
            limits,
        }
    }

    /// Add new transaction to the pool
    pub fn add(
        &mut self,
        tx_hash: TransactionHash,
        tx: OwnedTransaction,
    ) -> Result<(), TransactionAddError> {
        if self.contains(&tx_hash) {
            return Err(TransactionAddError::AlreadyExists);
        }

        let block_hash = tx.transaction().header.block_hash;
        let Some(block_txs) = self.by_block_hash.get_mut(&block_hash) else {
            return Err(TransactionAddError::BlockNotFound);
        };

        if self.transactions.len() == self.limits.count.get() {
            return Err(TransactionAddError::TooManyTransactions);
        }

        let tx_size = tx.buffer().len() as usize;
        if self.limits.size.get() - self.total_size < tx_size {
            return Err(TransactionAddError::TotalSizeTooLarge);
        }

        self.total_size += tx_size;
        self.transactions.insert(
            tx_hash,
            PoolTransaction {
                tx,
                state: TransactionState::New,
            },
        );
        block_txs.txs.insert(tx_hash);

        Ok(())
    }

    /// Mark transaction as authorized as of a specific block.
    ///
    /// Returns `false` if transaction is unknown.
    pub fn mark_authorized(
        &mut self,
        tx_hash: &TransactionHash,
        block_number: BlockNumber,
        block_hash: BlockHash,
    ) -> bool {
        let Some(tx) = self.transactions.get_mut(tx_hash) else {
            return false;
        };

        let authorized_details = TransactionAuthorizedDetails {
            block_number,
            block_hash,
        };

        match &mut tx.state {
            TransactionState::New => {
                tx.state = TransactionState::Authorized {
                    at: VecDeque::from([authorized_details]),
                };
            }
            TransactionState::Authorized { at } => {
                if at.len() == usize::from(self.authorization_history_depth.get()) {
                    at.pop_back();
                }
                at.push_front(authorized_details);
            }
        }

        true
    }

    /// Whether transaction pool contains a transaction
    pub fn contains(&self, tx_hash: &TransactionHash) -> bool {
        self.transactions.contains_key(tx_hash)
    }

    /// Get iterator over all transactions
    pub fn iter(
        &self,
    ) -> impl ExactSizeIterator<Item = (&'_ TransactionHash, &'_ PoolTransaction)> + '_ {
        self.transactions.iter()
    }

    /// Remove transactions from the pool
    pub fn remove<'a, Txs>(&mut self, tx_hashes: Txs)
    where
        Txs: Iterator<Item = &'a TransactionHash>,
    {
        for tx_hash in tx_hashes {
            self.remove_single_tx(tx_hash)
        }
    }

    fn remove_single_tx(&mut self, tx_hash: &TransactionHash) {
        if let Some(tx) = self.transactions.remove(tx_hash) {
            self.total_size -= tx.tx.buffer().len() as usize;

            let block_hash = &tx.tx.transaction().header.block_hash;
            if let Some(set) = self.by_block_hash.get_mut(block_hash) {
                set.txs.remove(tx_hash);
                if set.txs.is_empty() {
                    self.by_block_hash.remove(block_hash);
                }
            }
        }
    }

    /// Add the new best block.
    ///
    /// If there is already an existing block with the same or higher block number, it will be
    /// removed alongside all transactions. Blocks older than configured pruning depth will be
    /// removed automatically as well.
    ///
    /// This allows accepting transactions created at specified block hash.
    pub fn add_best_block(&mut self, block_number: BlockNumber, block_hash: BlockHash) {
        // Clean up old blocks or blocks that are at the same or higher block number
        let allowed_blocks = block_number.saturating_sub(self.pruning_depth.get())..block_number;
        self.by_block_number
            .retain(|existing_block_number, existing_block_hash| {
                if allowed_blocks.contains(existing_block_number) {
                    return true;
                }

                if let Some(tx_hashes) = self.by_block_hash.remove(existing_block_hash) {
                    for tx_hash in tx_hashes.txs {
                        if let Some(tx) = self.transactions.remove(&tx_hash) {
                            self.total_size -= tx.tx.buffer().len() as usize;
                        }
                    }
                }
                false
            });

        for tx in self.transactions.values_mut() {
            if let TransactionState::Authorized { at } = &mut tx.state {
                // Clean up verification status for blocks at the same or higher block number
                while at
                    .pop_front_if(|details| details.block_number >= block_number)
                    .is_some()
                {}
            }
        }

        self.by_block_number.insert(block_number, block_hash);
        self.by_block_hash.insert(
            block_hash,
            BlockHashDetails {
                txs: HashSet::new(),
            },
        );
    }
}
