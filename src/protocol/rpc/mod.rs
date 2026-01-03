//! RPC (Remote Procedure Call) protocol implementation as specified in RFC 5531 (previously RFC 1057).
//!
//! The RPC protocol enables programs to call procedures on remote systems as if
//! they were local calls. It forms the foundation for all NFS operations by
//! providing a standard mechanism for client-server communication.
//!
//! This module implements RPC version 2 with the following features:
//!
//! 1. Message framing for TCP using the Record Marking Standard
//! 2. Transaction tracking for detecting and handling retransmissions
//! 3. Authentication (`AUTH_UNIX`)
//! 4. Program/procedure number dispatching
//! 5. Error handling and reporting
//! 6. Asynchronous message processing
//! 7. Ordered command processing with FIFO guarantees
//!
//! RPC provides important benefits for distributed systems:
//! - Location transparency (clients don't need to know server locations)
//! - Network protocol independence (can run over TCP or UDP)
//! - Platform neutrality through XDR (External Data Representation)
//! - Built-in authentication and security mechanisms
//!
//! The implementation in this module serves as the communication layer for
//! the NFS, MOUNT, and PORTMAP protocols, handling all aspects of message
//! encoding, transmission, and routing.

mod command_queue;
mod context;
mod transaction_tracker;
mod wire;

pub use context::Context;
pub use transaction_tracker::{TransactionStatus, TransactionTracker};
pub use wire::{write_fragment, SocketMessageHandler};

/// Linux-compatible max NFS block size (RPCSVC_MAXPAYLOAD).
pub const MAX_BLOCK_SIZE: usize = 4 * 1024 * 1024;
/// Linux PAGE_SIZE, used to derive the max RPC record length.
pub const PAGE_SIZE: usize = 4096;
/// Max RPC record length for TCP (roundup(MAX_BLOCK_SIZE + PAGE_SIZE, PAGE_SIZE)).
pub const MAX_RPC_RECORD_LENGTH: usize =
    ((MAX_BLOCK_SIZE + PAGE_SIZE + PAGE_SIZE - 1) / PAGE_SIZE) * PAGE_SIZE;
