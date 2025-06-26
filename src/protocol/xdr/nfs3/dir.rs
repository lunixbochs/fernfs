//! Module contains XDR data structures related to directories for NFS version 3 protocol
//! as defined in RFC 1813.
//!
//! This module includes data structures for the following directory operations:
//! - MKDIR: Create a directory (procedure 9)
//! - SYMLINK: Create a symbolic link (procedure 10)
//! - READDIR: Read from a directory (procedure 16)
//! - READDIRPLUS: Extended read from a directory (procedure 17)
//! - MKNOD: Create a special device (procedure 11)
//!
//! These structures implement the XDR serialization/deserialization interfaces for
//! the request arguments and response data of directory-related operations.

// Allow unused code warnings since we implement the complete RFC 1813 specification,
// including procedures that may not be used by all clients
#![allow(dead_code)]
// Preserve original RFC naming conventions (e.g. READDIR3args, MKDIR3resok)
// for consistency with the NFS version 3 protocol specification
#![allow(non_camel_case_types)]

use std::io::{Read, Write};

use num_derive::{FromPrimitive, ToPrimitive};

use super::*;

/// Enumeration of device types for special files in NFS version 3
/// as defined in RFC 1813 section 3.3.11
/// Used to identify the type of device when creating special files
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug, Default, FromPrimitive, ToPrimitive)]
#[repr(u32)]
pub enum devicetype3 {
    /// Character special device
    #[default]
    NF3CHR = 0,
    /// Block special device
    NF3BLK = 1,
    /// Socket
    NF3SOCK = 2,
    /// FIFO pipe
    NF3FIFO = 3,
}
impl SerializeEnum for devicetype3 {}
impl DeserializeEnum for devicetype3 {}

/// Arguments for the MKDIR procedure (procedure 9)
/// as defined in RFC 1813 section 3.3.9
/// Used to create a new directory
#[allow(non_camel_case_types)]
#[derive(Debug, Default)]
pub struct MKDIR3args {
    /// Directory where new directory should be created and its name
    pub dirops: diropargs3,
    /// Initial attributes for the new directory
    pub attributes: sattr3,
}
DeserializeStruct!(MKDIR3args, dirops, attributes);
SerializeStruct!(MKDIR3args, dirops, attributes);

/// Arguments for the SYMLINK procedure (procedure 10)
/// as defined in RFC 1813 section 3.3.10
/// Used to create a symbolic link
#[allow(non_camel_case_types)]
#[derive(Debug, Default)]
pub struct SYMLINK3args {
    /// Directory where symbolic link should be created and its name
    pub dirops: diropargs3,
    /// Target path and attributes for the symbolic link
    pub symlink: symlinkdata3,
}
DeserializeStruct!(SYMLINK3args, dirops, symlink);
SerializeStruct!(SYMLINK3args, dirops, symlink);

/// Directory entry returned by READDIR operation
/// as defined in RFC 1813 section 3.3.16
#[allow(non_camel_case_types)]
#[derive(Debug, Default)]
pub struct entry3 {
    /// File identifier (inode number)
    pub fileid: fileid3,
    /// Name of the directory entry
    pub name: filename3,
    /// Cookie for the next READDIR operation
    pub cookie: cookie3,
}
DeserializeStruct!(entry3, fileid, name, cookie);
SerializeStruct!(entry3, fileid, name, cookie);

/// Arguments for the READDIR procedure (procedure 16)
/// as defined in RFC 1813 section 3.3.16
/// Used to read entries from a directory. The server returns a variable number of directory entries,
/// up to the specified count limit.
#[allow(non_camel_case_types)]
#[derive(Debug, Default)]
pub struct READDIR3args {
    /// File handle for the directory to be read
    pub dir: nfs_fh3,
    /// Cookie indicating where to start reading directory entries
    /// A cookie value of 0 means start at beginning of directory
    pub cookie: cookie3,
    /// Cookie verifier to detect whether directory has changed
    pub cookieverf: cookieverf3,
    /// Maximum number of bytes of directory information to return
    pub dircount: count3,
}
DeserializeStruct!(READDIR3args, dir, cookie, cookieverf, dircount);
SerializeStruct!(READDIR3args, dir, cookie, cookieverf, dircount);

/// Directory entry with additional attributes for READDIRPLUS operation
/// as defined in RFC 1813 section 3.3.17
/// This structure represents a single directory entry with extended information
#[allow(non_camel_case_types)]
#[derive(Debug, Default)]
pub struct entryplus3 {
    /// File identifier (inode number) uniquely identifying the file within the filesystem
    pub fileid: fileid3,
    /// Name of the directory entry (filename)
    pub name: filename3,
    /// Cookie value that can be used in subsequent READDIRPLUS calls to resume listing
    pub cookie: cookie3,
    /// File attributes for this directory entry
    pub name_attributes: post_op_attr,
    /// File handle for this directory entry
    pub name_handle: post_op_fh3,
}
DeserializeStruct!(entryplus3, fileid, name, cookie, name_attributes, name_handle);
SerializeStruct!(entryplus3, fileid, name, cookie, name_attributes, name_handle);

/// Arguments for the READDIRPLUS procedure (procedure 17)
/// as defined in RFC 1813 section 3.3.17
/// READDIRPLUS returns directory entries along with their attributes and file handles.
#[allow(non_camel_case_types)]
#[derive(Debug, Default)]
pub struct READDIRPLUS3args {
    /// Directory file handle
    pub dir: nfs_fh3,
    /// Cookie from previous READDIRPLUS - where to start reading
    pub cookie: cookie3,
    /// Cookie verifier to detect changed directories
    pub cookieverf: cookieverf3,
    /// Maximum number of bytes of directory information to return
    pub dircount: count3,
    /// Maximum number of bytes of attribute information to return
    pub maxcount: count3,
}
DeserializeStruct!(READDIRPLUS3args, dir, cookie, cookieverf, dircount, maxcount);
SerializeStruct!(READDIRPLUS3args, dir, cookie, cookieverf, dircount, maxcount);

/// Arguments for the MKNOD procedure (procedure 11)
/// as defined in RFC 1813 section 3.3.11
/// Used to create a special device file, FIFO, or socket
#[allow(non_camel_case_types)]
#[derive(Debug, Default)]
pub struct MKNOD3args {
    /// Directory where the special file should be created and its name
    pub where_dir: diropargs3,
    /// Type and device information for the special file
    pub what: mknoddata3,
}
DeserializeStruct!(MKNOD3args, where_dir, what);
SerializeStruct!(MKNOD3args, where_dir, what);

/// Device data for special files
/// as defined in RFC 1813 section 3.3.11
/// Contains the device type and device numbers
#[allow(non_camel_case_types)]
#[derive(Debug, Default)]
pub struct devicedata3 {
    /// Type of device (character, block, socket, or FIFO)
    pub dev_type: devicetype3,
    /// Major and minor device numbers for character and block devices
    pub device: specdata3,
}
DeserializeStruct!(devicedata3, dev_type, device);
SerializeStruct!(devicedata3, dev_type, device);

/// Data structure for creating special files
/// as defined in RFC 1813 section 3.3.11
/// Contains the file type and device information
#[allow(non_camel_case_types)]
#[derive(Debug, Default)]
pub struct mknoddata3 {
    /// Type of file to create (regular, directory, special file etc)
    pub mknod_type: ftype3,
    /// Device information if creating a special file
    pub device: devicedata3,
}
DeserializeStruct!(mknoddata3, mknod_type, device);
SerializeStruct!(mknoddata3, mknod_type, device);
