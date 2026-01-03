//! FernFS - A Network File System (NFS) server implementation in Rust
//!
//! This library provides a complete implementation of the NFS version 3 protocol
//! as defined in RFC 1813, allowing any Rust application to expose file systems
//! over the network to NFS clients.
//!
//! ## Supported Features
//!
//! - Full `NFSv3` protocol implementation (all 21 procedures defined in RFC 1813)
//! - `MOUNT` protocol for filesystem exports
//! - `PORTMAP` protocol for service discovery
//! - `TCP` and `UDP` transport protocols
//! - Asynchronous operation with Tokio runtime
//! - Virtual File System abstraction for implementing custom backends
//!
//! ## Main Components
//!
//! - `vfs`: The Virtual File System API that must be implemented to create a custom NFS-exportable
//!   file system. This abstracts the underlying storage from the NFS protocol details.
//!
//! - `tcp`: TCP-based server implementation that handles client connections and dispatches
//!   NFS protocol requests to the appropriate handlers.
//!
//! - `protocol`: Internal module that implements the `NFS`, `MOUNT`, and `PORTMAP` protocols,
//!   including `XDR` (External Data Representation) encoding/decoding.
//!
//! - `fs_util`: Utility functions for working with file systems.
//!
//! ## Standards Compliance
//!
//! This implementation follows these RFCs:
//! - RFC 1813: NFS Version 3 Protocol Specification
//! - RFC 5531: RPC: Remote Procedure Call Protocol Specification Version 2 (obsoletes RFC 1831)
//! - RFC 1832: XDR: External Data Representation Standard (obsoletes RFC 1014)
//! - RFC 1833: Binding Protocols for ONC RPC Version 2
//!
//! ## Usage
//!
//! To create an NFS server, implement the `NFSFileSystem` trait and use the `NFSTcpListener`
//! to expose it over the network.

pub mod protocol;
mod write_counter;

#[cfg(not(target_os = "windows"))]
pub mod fs_util;

pub mod tcp;
pub mod vfs;

pub use protocol::xdr;
