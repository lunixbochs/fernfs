//! This module implements the MOUNT protocol (RFC 1813 Appendix I) data structures
//! for XDR serialization and deserialization.
//!
//! The MOUNT protocol is used to establish the initial connection between an NFS client
//! and server. It provides functions for mounting and unmounting file systems, and
//! obtaining the initial file handle that serves as the root of the mounted file system.

// Allow unused code since we implement the complete RFC specification
#![allow(dead_code)]
// Keep original RFC naming conventions for consistency with the specification
#![allow(non_camel_case_types)]

use std::io::{Read, Write};

use crate::xdr::{DeserializeEnum, SerializeEnum};
use num_derive::{FromPrimitive, ToPrimitive};

use super::*;

/// MOUNT program number for RPC
pub const PROGRAM: u32 = 100005;
/// MOUNT protocol version 3
pub const VERSION: u32 = 3;

/// Maximum bytes in a path name
pub const MNTPATHLEN: u32 = 1024;
/// Maximum bytes in a name
pub const MNTNAMLEN: u32 = 255;
/// Maximum bytes in a V3 file handle
pub const FHSIZE3: u32 = 64;

/// File handle for NFS version 3
pub type fhandle3 = Vec<u8>;
/// Directory path on the server
pub type dirpath = Vec<u8>;
/// Name in the directory
pub type name = Vec<u8>;

/// Status codes returned by MOUNT protocol operations
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug, FromPrimitive, ToPrimitive)]
#[repr(u32)]
pub enum mountstat3 {
    /// No error
    MNT3_OK = 0, /* no error */
    /// Not owner
    MNT3ERR_PERM = 1, /* Not owner */
    /// No such file or directory
    MNT3ERR_NOENT = 2, /* No such file or directory */
    /// I/O error
    MNT3ERR_IO = 5, /* I/O error */
    /// Permission denied
    MNT3ERR_ACCES = 13, /* Permission denied */
    /// Not a directory
    MNT3ERR_NOTDIR = 20, /* Not a directory */
    /// Invalid argument
    MNT3ERR_INVAL = 22, /* Invalid argument */
    /// Filename too long
    MNT3ERR_NAMETOOLONG = 63, /* Filename too long */
    /// Operation not supported
    MNT3ERR_NOTSUPP = 10004, /* Operation not supported */
    /// A failure on the server
    MNT3ERR_SERVERFAULT = 10006, /* A failure on the server */
}
impl SerializeEnum for mountstat3 {}
impl DeserializeEnum for mountstat3 {}

/// Successful response to a mount request
#[allow(non_camel_case_types)]
#[derive(Clone, Debug)]
pub struct mountres3_ok {
    /// File handle for the mounted directory
    pub fhandle: fhandle3, // really same thing as nfs::nfs_fh3
    /// List of authentication flavors supported by the server
    pub auth_flavors: Vec<u32>,
}
DeserializeStruct!(mountres3_ok, fhandle, auth_flavors);
SerializeStruct!(mountres3_ok, fhandle, auth_flavors);

/// Procedure numbers for the MOUNT version 3 protocol
#[allow(non_camel_case_types)]
#[allow(clippy::upper_case_acronyms)]
#[derive(Copy, Clone, Debug, FromPrimitive, ToPrimitive)]
pub enum MountProgram {
    /// Null procedure for service availability testing
    MOUNTPROC3_NULL = 0,
    /// Mount a file system
    MOUNTPROC3_MNT = 1,
    /// Get list of mounted file systems
    MOUNTPROC3_DUMP = 2,
    /// Unmount a file system
    MOUNTPROC3_UMNT = 3,
    /// Unmount all file systems
    MOUNTPROC3_UMNTALL = 4,
    /// Get list of exported file systems
    MOUNTPROC3_EXPORT = 5,
    /// Invalid procedure number
    INVALID,
}
impl SerializeEnum for MountProgram {}
impl DeserializeEnum for MountProgram {}
