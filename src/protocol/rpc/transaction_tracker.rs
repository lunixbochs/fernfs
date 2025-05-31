//! Transaction tracking for RPC idempotency as described in RFC 5531 (previously RFC 1057).
//!
//! This module implements the idempotency requirements for RPC by tracking
//! transaction state using transaction IDs (XIDs) and client addresses.
//! It ensures that:
//!
//! - Duplicate requests due to network retransmissions are properly identified
//! - Only one instance of a given RPC request is processed
//! - Transaction state is maintained for a configurable period to handle delayed retransmissions
//! - Server resources are managed efficiently by cleaning up expired transaction records
//!
//! The transaction tracking system is essential for maintaining the at-most-once
//! semantics required by NFS and other RPC-based protocols, where duplicate
//! operations (like file writes) could cause data corruption.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

/// Tracks RPC transactions to detect and handle retransmissions
///
/// Implements idempotency for RPC operations by tracking transaction state
/// using a combination of transaction ID (XID) and client address.
/// Helps prevent duplicate processing of retransmitted requests
/// and maintains transaction state for a configurable retention period.
pub struct TransactionTracker {
    retention_period: Duration,
    transactions: Mutex<HashMap<(u32, String), TransactionState>>,
}

impl TransactionTracker {
    /// Creates a new transaction tracker with specified retention period
    ///
    /// Initializes a transaction tracker that will maintain transaction state
    /// for the given duration. This helps balance memory usage with the ability
    /// to detect retransmissions over time.
    pub fn new(retention_period: Duration) -> Self {
        Self { retention_period, transactions: Mutex::new(HashMap::new()) }
    }

    /// Checks if a transaction is a retransmission
    ///
    /// Identifies whether the transaction with given XID and client address
    /// has been seen before. If it's a new transaction, marks it as in-progress.
    /// Returns true for retransmissions, false for new transactions.
    pub fn is_retransmission(&self, xid: u32, client_addr: &str) -> bool {
        let key = (xid, client_addr.to_string());
        let mut transactions =
            self.transactions.lock().expect("unable to unlock transactions mutex");
        housekeeping(&mut transactions, self.retention_period);
        if let std::collections::hash_map::Entry::Vacant(e) = transactions.entry(key) {
            e.insert(TransactionState::InProgress);
            false
        } else {
            true
        }
    }

    /// Marks a transaction as successfully processed
    ///
    /// Updates the state of a transaction from in-progress to completed,
    /// recording the completion time for retention period calculations.
    /// Called after a transaction has been fully processed and responded to.
    pub fn mark_processed(&self, xid: u32, client_addr: &str) {
        let key = (xid, client_addr.to_string());
        let completion_time = SystemTime::now();
        let mut transactions =
            self.transactions.lock().expect("unable to unlock transactions mutex");
        if let Some(tx) = transactions.get_mut(&key) {
            *tx = TransactionState::Completed(completion_time);
        }
    }
}

/// Removes expired transactions from the tracking map
///
/// Cleans up completed transactions that have exceeded the maximum retention age.
/// Keeps in-progress transactions regardless of age to prevent processing duplicates.
/// Called during transaction checks to maintain memory efficiency.
fn housekeeping(transactions: &mut HashMap<(u32, String), TransactionState>, max_age: Duration) {
    let mut cutoff = SystemTime::now() - max_age;
    transactions.retain(|_, v| match v {
        TransactionState::InProgress => true,
        TransactionState::Completed(completion_time) => completion_time >= &mut cutoff,
    });
}

/// Represents the current state of an RPC transaction
///
/// Either in-progress (currently being processed) or
/// completed (successfully processed with timestamp).
/// Used for tracking transaction lifecycle and retransmission detection.
enum TransactionState {
    InProgress,
    Completed(SystemTime),
}
