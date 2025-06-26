//! This module implements the Portmap protocol (RFC 5531, previously RFC 1057 Appendix A) data structures
//! for XDR serialization and deserialization.
//!
//! Portmap (also known as Portmapper) is a service that maps RPC program numbers
//! to network port numbers. Clients use the Portmapper to locate the port number
//! for a specific RPC service they wish to use.

// Allow unused code since we implement the complete RFC specification
#![allow(dead_code)]
// Keep original RFC naming conventions for consistency with the specification
#![allow(non_camel_case_types)]

use std::io::{Read, Write};

use num_derive::{FromPrimitive, ToPrimitive};

use super::*;

/// Represents a mapping between an RPC program and a network port.
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug, Default)]
#[repr(C)]
pub struct mapping {
    /// The RPC program number
    pub prog: u32,
    /// The RPC program version number
    pub vers: u32,
    /// The transport protocol (TCP or UDP, see IPPROTO_* constants)
    pub prot: u32,
    /// The port number where the service is listening
    pub port: u32,
}
DeserializeStruct!(mapping, prog, vers, prot, port);
SerializeStruct!(mapping, prog, vers, prot, port);

/// Protocol number for TCP/IP
pub const IPPROTO_TCP: u32 = 6;
/// Protocol number for UDP/IP
pub const IPPROTO_UDP: u32 = 17;
/// Portmap RPC program number
pub const PROGRAM: u32 = 100000;
/// Portmap RPC version number
pub const VERSION: u32 = 2;

/// Procedure numbers for the Portmap RPC service.
#[allow(non_camel_case_types)]
#[allow(clippy::upper_case_acronyms)]
#[derive(Copy, Clone, Debug, FromPrimitive, ToPrimitive)]
pub enum PortmapProgram {
    /// Null procedure for service availability testing
    PMAPPROC_NULL = 0,
    /// Register a new program-to-port mapping
    PMAPPROC_SET = 1,
    /// Remove a program-to-port mapping
    PMAPPROC_UNSET = 2,
    /// Look up the port for a program
    PMAPPROC_GETPORT = 3,
    /// List all registered program-to-port mappings
    PMAPPROC_DUMP = 4,
    /// Call another registered procedure
    PMAPPROC_CALLIT = 5,
    /// Invalid procedure number
    INVALID,
}
impl SerializeEnum for PortmapProgram {}
impl DeserializeEnum for PortmapProgram {}
