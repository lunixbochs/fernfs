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
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

/// Tracks RPC transactions to detect and handle retransmissions
///
/// Implements idempotency for RPC operations by tracking transaction state
/// using a combination of transaction ID (XID) and client address.
/// Helps prevent duplicate processing of retransmitted requests
/// and maintains transaction state for a configurable retention period.
pub struct TransactionTracker {
    retention_period: Duration,
    housekeeping_interval: Duration,
    transactions: Mutex<TransactionStore>,
}

struct TransactionStore {
    transactions: HashMap<(u32, String), TransactionState>,
    last_housekeeping: SystemTime,
}

/// Status of a tracked transaction.
#[derive(Debug)]
pub enum TransactionStatus {
    /// First time seeing this transaction; it is now marked in-progress.
    New,
    /// A request with the same XID is already in progress.
    InProgress,
    /// A completed request with a cached response.
    Completed(Arc<Vec<u8>>),
}

impl TransactionTracker {
    /// Creates a new transaction tracker with specified retention period
    ///
    /// Initializes a transaction tracker that will maintain transaction state
    /// for the given duration. This helps balance memory usage with the ability
    /// to detect retransmissions over time.
    pub fn new(retention_period: Duration) -> Self {
        let housekeeping_interval = if retention_period < Duration::from_secs(1) {
            retention_period
        } else {
            std::cmp::max(Duration::from_secs(1), retention_period / 4)
        };
        Self {
            retention_period,
            housekeeping_interval,
            transactions: Mutex::new(TransactionStore {
                transactions: HashMap::new(),
                last_housekeeping: SystemTime::now(),
            }),
        }
    }

    /// Checks transaction status and records new calls as in-progress.
    ///
    /// Identifies whether the transaction with given XID and client address
    /// has been seen before. If it's a new transaction, marks it as in-progress.
    pub fn check(&self, xid: u32, client_addr: &str) -> TransactionStatus {
        let key = (xid, client_addr.to_string());
        let now = SystemTime::now();
        let mut store = self.transactions.lock().expect("unable to unlock transactions mutex");
        if !store.transactions.is_empty()
            && should_housekeep(store.last_housekeeping, now, self.housekeeping_interval)
        {
            housekeeping(&mut store.transactions, self.retention_period, now);
            store.last_housekeeping = now;
        }
        match store.transactions.entry(key) {
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert(TransactionState::InProgress);
                TransactionStatus::New
            }
            std::collections::hash_map::Entry::Occupied(mut entry) => match entry.get_mut() {
                TransactionState::InProgress => TransactionStatus::InProgress,
                TransactionState::Completed { completion_time, response } => {
                    *completion_time = now;
                    TransactionStatus::Completed(Arc::clone(response))
                }
            },
        }
    }

    /// Records a completed transaction response for retransmission replay.
    ///
    /// Updates the state of a transaction from in-progress to completed,
    /// recording the completion time for retention period calculations.
    /// Called after a transaction has been fully processed and responded to.
    pub fn record_response(&self, xid: u32, client_addr: &str, response: Arc<Vec<u8>>) {
        let key = (xid, client_addr.to_string());
        let completion_time = SystemTime::now();
        let mut store = self.transactions.lock().expect("unable to unlock transactions mutex");
        store.transactions.insert(key, TransactionState::Completed { completion_time, response });
    }

    /// Clears a transaction entry so a later retry can be processed.
    pub fn clear(&self, xid: u32, client_addr: &str) {
        let key = (xid, client_addr.to_string());
        let mut store = self.transactions.lock().expect("unable to unlock transactions mutex");
        store.transactions.remove(&key);
    }
}

fn should_housekeep(last: SystemTime, now: SystemTime, interval: Duration) -> bool {
    now.duration_since(last).map(|elapsed| elapsed >= interval).unwrap_or(true)
}

/// Removes expired transactions from the tracking map
///
/// Cleans up completed transactions that have exceeded the maximum retention age.
/// Keeps in-progress transactions regardless of age to prevent processing duplicates.
/// Called during transaction checks to maintain memory efficiency.
fn housekeeping(
    transactions: &mut HashMap<(u32, String), TransactionState>,
    max_age: Duration,
    now: SystemTime,
) {
    let mut cutoff = now - max_age;
    transactions.retain(|_, v| match v {
        TransactionState::InProgress => true,
        TransactionState::Completed { completion_time, .. } => completion_time >= &mut cutoff,
    });
}

/// Represents the current state of an RPC transaction
///
/// Either in-progress (currently being processed) or
/// completed (successfully processed with timestamp).
/// Used for tracking transaction lifecycle and retransmission detection.
enum TransactionState {
    InProgress,
    Completed { completion_time: SystemTime, response: Arc<Vec<u8>> },
}

#[cfg(test)]
mod tests {
    use super::{TransactionStatus, TransactionTracker};
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn retransmit_in_flight_reports_in_progress() {
        let tracker = TransactionTracker::new(Duration::from_secs(60));
        let xid = 7;
        let client_addr = "127.0.0.1:1234";

        assert!(matches!(tracker.check(xid, client_addr), TransactionStatus::New));
        assert!(matches!(tracker.check(xid, client_addr), TransactionStatus::InProgress));

        let response = Arc::new(vec![1, 2, 3]);
        tracker.record_response(xid, client_addr, Arc::clone(&response));
        match tracker.check(xid, client_addr) {
            TransactionStatus::Completed(replay) => {
                assert_eq!(&*replay, &*response);
            }
            other => panic!("expected Completed, got {other:?}"),
        }
    }
}
