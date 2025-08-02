//! NFS protocol implementation module.
//!
//! This module provides implementations of the main protocols required for an NFS server:
//!
//! - `v3`: The NFS version 3 protocol as specified in RFC 1813. This includes all
//!   procedure handlers for the 21 operations defined in the protocol, such as
//!   `READ`, `WRITE`, `LOOKUP`, `CREATE`, etc.
//!
//! - `mount`: The `MOUNT` protocol implementation, which allows clients to mount
//!   file systems exported by the server. This protocol is a prerequisite for using
//!   NFS as it provides the initial file handle for the mount point.
//!
//! - `portmap`: The `PORTMAP` protocol (also known as `RPCBIND`) implementation, which
//!   allows clients to discover which port numbers are assigned to specific RPC programs.
//!   This is used by clients to locate the NFS and `MOUNT` services.
//!
//! Together, these protocols form a complete NFS version 3 service as defined by
//! the relevant RFCs. The NFS protocol is designed to be transport-independent,
//! though in this implementation it is primarily used over TCP.

pub mod mount;
pub mod portmap;
pub mod v3;
pub mod v4;
