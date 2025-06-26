//! The module defines XDR data types and constants for Network File System (NFS)
//! version 3, as defined in RFC 1813.
//!
//! NFS version 3 is a stateless distributed file system protocol
//! that provides transparent remote access to shared file systems over a network.
//! This implementation provides the data structures needed for encoding and
//! decoding NFS version 3 protocol messages using XDR (External Data Representation).
//!
//! This module defines the constants, basic data types, and complex structures
//! that form the foundation of the NFSv3 protocol as specified in RFC 1813.

// Allow unused code since we're implementing the full NFS3 protocol specification
#![allow(dead_code)]
// Preserve original RFC naming conventions for consistency with the specification
#![allow(non_camel_case_types)]

use std::fmt;
use std::io::{Read, Write};

use filetime;
use num_derive::{FromPrimitive, ToPrimitive};

use crate::xdr::{DeserializeEnum, SerializeEnum};
use crate::{DeserializeStruct, SerializeStruct};

use super::{deserialize, Deserialize, Serialize};

// Modules for different operation types
pub mod dir;
pub mod file;
pub mod fs;

// Section 2.2 Constants
/// The RPC program number for NFS version 3 service.
pub const PROGRAM: u32 = 100003;
/// The version number for NFS version 3 protocol.
pub const VERSION: u32 = 3;

// Section 2.4 Sizes
//
/// The maximum size in bytes of the opaque file handle.
pub const NFS3_FHSIZE: u32 = 64;

/// The size in bytes of the opaque cookie verifier passed by
/// READDIR and READDIRPLUS.
pub const NFS3_COOKIEVERFSIZE: u32 = 8;

/// The size in bytes of the opaque verifier used for
/// exclusive CREATE.
pub const NFS3_CREATEVERFSIZE: u32 = 8;

/// The size in bytes of the opaque verifier used for
/// asynchronous WRITE.
pub const NFS3_WRITEVERFSIZE: u32 = 8;

// Section 2.5 Basic Data Types
/// A string type used in NFS for filenames and paths.
///
/// This is essentially a vector of bytes, but with specific
/// formatting for NFS protocol requirements.
#[allow(non_camel_case_types)]
#[derive(Default, Clone)]
pub struct nfsstring(pub Vec<u8>);

impl nfsstring {
    /// Returns the length of the string in bytes.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if the string is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<Vec<u8>> for nfsstring {
    fn from(value: Vec<u8>) -> Self {
        Self(value)
    }
}

impl From<&[u8]> for nfsstring {
    fn from(value: &[u8]) -> Self {
        Self(value.into())
    }
}

impl AsRef<[u8]> for nfsstring {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl std::ops::Deref for nfsstring {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl fmt::Debug for nfsstring {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", String::from_utf8_lossy(&self.0))
    }
}

impl fmt::Display for nfsstring {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", String::from_utf8_lossy(&self.0))
    }
}

impl Serialize for nfsstring {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        self.0.serialize(dest)
    }
}

impl Deserialize for nfsstring {
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        self.0.deserialize(src)
    }
}

/// Procedure numbers for NFS version 3 protocol.
#[allow(non_camel_case_types)]
#[allow(clippy::upper_case_acronyms)]
#[derive(Copy, Clone, Debug, FromPrimitive, ToPrimitive)]
pub enum NFSProgram {
    /// Do nothing - used primarily for performance measurement
    NFSPROC3_NULL = 0,
    /// Get file attributes
    NFSPROC3_GETATTR = 1,
    /// Set file attributes
    NFSPROC3_SETATTR = 2,
    /// Look up filename
    NFSPROC3_LOOKUP = 3,
    /// Check file access permission
    NFSPROC3_ACCESS = 4,
    /// Read from symbolic link
    NFSPROC3_READLINK = 5,
    /// Read from file
    NFSPROC3_READ = 6,
    /// Write to file
    NFSPROC3_WRITE = 7,
    /// Create file
    NFSPROC3_CREATE = 8,
    /// Create directory
    NFSPROC3_MKDIR = 9,
    /// Create symbolic link
    NFSPROC3_SYMLINK = 10,
    /// Create special device
    NFSPROC3_MKNOD = 11,
    /// Remove file
    NFSPROC3_REMOVE = 12,
    /// Remove directory
    NFSPROC3_RMDIR = 13,
    /// Rename file or directory
    NFSPROC3_RENAME = 14,
    /// Create hard link
    NFSPROC3_LINK = 15,
    /// Read directory
    NFSPROC3_READDIR = 16,
    /// Extended read directory
    NFSPROC3_READDIRPLUS = 17,
    /// Get file system statistics
    NFSPROC3_FSSTAT = 18,
    /// Get file system information
    NFSPROC3_FSINFO = 19,
    /// Get path configuration
    NFSPROC3_PATHCONF = 20,
    /// Commit cached data
    NFSPROC3_COMMIT = 21,
    /// Invalid procedure
    INVALID = 22,
}

/// Opaque byte type as defined in RFC 1813 section 2.5
/// Used for binary data like file handles and verifiers
pub type opaque = u8;
/// Filename type as defined in RFC 1813 section 2.5
/// String used for a component of a pathname
pub type filename3 = nfsstring;
/// Path type as defined in RFC 1813 section 2.5
/// String used for a pathname or a symbolic link contents
pub type nfspath3 = nfsstring;
/// File identifier as defined in RFC 1813 section 2.5
/// A unique number that identifies a file within a filesystem
pub type fileid3 = u64;
/// Directory entry position cookie as defined in RFC 1813 section 2.5
/// Used in READDIR and READDIRPLUS operations for iteration
pub type cookie3 = u64;
/// Cookie verifier for directory operations as defined in RFC 1813 section 2.5
/// Used to detect when a directory being read has changed
pub type cookieverf3 = [opaque; NFS3_COOKIEVERFSIZE as usize];
/// Create verifier for exclusive file creation as defined in RFC 1813 section 2.5
/// Used in CREATE operations with EXCLUSIVE mode to ensure uniqueness
pub type createverf3 = [opaque; NFS3_CREATEVERFSIZE as usize];
/// Write verifier for asynchronous writes as defined in RFC 1813 section 2.5
/// Used to detect server reboots between asynchronous WRITE and COMMIT operations
pub type writeverf3 = [opaque; NFS3_WRITEVERFSIZE as usize];
/// User ID as defined in RFC 1813 section 2.5
/// Identifies the owner of a file
pub type uid3 = u32;
/// Group ID as defined in RFC 1813 section 2.5
/// Identifies the group ownership of a file
pub type gid3 = u32;
/// File size in bytes as defined in RFC 1813 section 2.5
pub type size3 = u64;
/// File offset in bytes as defined in RFC 1813 section 2.5
/// Used to specify a position within a file
pub type offset3 = u64;
/// File mode bits as defined in RFC 1813 section 2.5
/// Contains file type and permission bits
pub type mode3 = u32;
/// Count of bytes or entries as defined in RFC 1813 section 2.5
/// Used for various counting purposes in NFS operations
pub type count3 = u32;

/// Status codes returned by NFS version 3 operations
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug, FromPrimitive, ToPrimitive)]
#[repr(u32)]
pub enum nfsstat3 {
    /// Indicates the call completed successfully.
    NFS3_OK = 0,
    /// Not owner. The operation was not allowed because the
    /// caller is either not a privileged user (root) or not the
    /// owner of the target of the operation.
    NFS3ERR_PERM = 1,
    /// No such file or directory. The file or directory name
    /// specified does not exist.
    NFS3ERR_NOENT = 2,
    /// I/O error. A hard error (for example, a disk error)
    /// occurred while processing the requested operation.
    NFS3ERR_IO = 5,
    /// I/O error. No such device or address.
    NFS3ERR_NXIO = 6,
    /// Permission denied. The caller does not have the correct
    /// permission to perform the requested operation. Contrast
    /// this with NFS3ERR_PERM, which restricts itself to owner
    /// or privileged user permission failures.
    NFS3ERR_ACCES = 13,
    /// File exists. The file specified already exists.
    NFS3ERR_EXIST = 17,
    /// Attempt to do a cross-device hard link.
    NFS3ERR_XDEV = 18,
    /// No such device.
    NFS3ERR_NODEV = 19,
    /// Not a directory. The caller specified a non-directory in
    /// a directory operation.
    NFS3ERR_NOTDIR = 20,
    /// Is a directory. The caller specified a directory in a
    /// non-directory operation.
    NFS3ERR_ISDIR = 21,
    /// Invalid argument or unsupported argument for an
    /// operation. Two examples are attempting a READLINK on an
    /// object other than a symbolic link or attempting to
    /// SETATTR a time field on a server that does not support
    /// this operation.
    NFS3ERR_INVAL = 22,
    /// File too large. The operation would have caused a file to
    /// grow beyond the server's limit.
    NFS3ERR_FBIG = 27,
    /// No space left on device. The operation would have caused
    /// the server's file system to exceed its limit.
    NFS3ERR_NOSPC = 28,
    /// Read-only file system. A modifying operation was
    /// attempted on a read-only file system.
    NFS3ERR_ROFS = 30,
    /// Too many hard links.
    NFS3ERR_MLINK = 31,
    /// The filename in an operation was too long.
    NFS3ERR_NAMETOOLONG = 63,
    /// An attempt was made to remove a directory that was not empty.
    NFS3ERR_NOTEMPTY = 66,
    /// Resource (quota) hard limit exceeded. The user's resource
    /// limit on the server has been exceeded.
    NFS3ERR_DQUOT = 69,
    /// Invalid file handle. The file handle given in the
    /// arguments was invalid. The file referred to by that file
    /// handle no longer exists or access to it has been
    /// revoked.
    NFS3ERR_STALE = 70,
    /// Too many levels of remote in path. The file handle given
    /// in the arguments referred to a file on a non-local file
    /// system on the server.
    NFS3ERR_REMOTE = 71,
    /// Illegal NFS file handle. The file handle failed internal
    /// consistency checks.
    NFS3ERR_BADHANDLE = 10001,
    /// Update synchronization mismatch was detected during a
    /// SETATTR operation.
    NFS3ERR_NOT_SYNC = 10002,
    /// READDIR or READDIRPLUS cookie is stale
    NFS3ERR_BAD_COOKIE = 10003,
    /// Operation is not supported.
    NFS3ERR_NOTSUPP = 10004,
    /// Buffer or request is too small.
    NFS3ERR_TOOSMALL = 10005,
    /// An error occurred on the server which does not map to any
    /// of the legal NFS version 3 protocol error values.  The
    /// client should translate this into an appropriate error.
    /// UNIX clients may choose to translate this to EIO.
    NFS3ERR_SERVERFAULT = 10006,
    /// An attempt was made to create an object of a type not
    /// supported by the server.
    NFS3ERR_BADTYPE = 10007,
    /// The server initiated the request, but was not able to
    /// complete it in a timely fashion. The client should wait
    /// and then try the request with a new RPC transaction ID.
    /// For example, this error should be returned from a server
    /// that supports hierarchical storage and receives a request
    /// to process a file that has been migrated. In this case,
    /// the server should start the immigration process and
    /// respond to client with this error.
    NFS3ERR_JUKEBOX = 10008,
}
impl SerializeEnum for nfsstat3 {}
impl DeserializeEnum for nfsstat3 {}

/// File type enumeration as defined in RFC 1813 section 2.3.5
/// Determines the type of a file system object
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug, Default, FromPrimitive, ToPrimitive)]
#[repr(u32)]
pub enum ftype3 {
    /// Regular File
    #[default]
    NF3REG = 1,
    /// Directory
    NF3DIR = 2,
    /// Block Special Device
    NF3BLK = 3,
    /// Character Special Device
    NF3CHR = 4,
    /// Symbolic Link
    NF3LNK = 5,
    /// Socket
    NF3SOCK = 6,
    /// Named Pipe
    NF3FIFO = 7,
}
impl SerializeEnum for ftype3 {}
impl DeserializeEnum for ftype3 {}

/// Special device information for character and block special devices
/// Contains the major and minor device numbers
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug, Default)]
pub struct specdata3 {
    /// Major device number
    pub specdata1: u32,
    /// Minor device number
    pub specdata2: u32,
}
DeserializeStruct!(specdata3, specdata1, specdata2);
SerializeStruct!(specdata3, specdata1, specdata2);

/// The NFS version 3 file handle
/// The file handle uniquely identifies a file or directory on the server
/// The server is responsible for the internal format and interpretation of the file handle
#[allow(non_camel_case_types)]
#[derive(Clone, Debug, Default)]
pub struct nfs_fh3 {
    /// Raw file handle data (up to NFS3_FHSIZE bytes)
    pub data: Vec<u8>,
}
DeserializeStruct!(nfs_fh3, data);
SerializeStruct!(nfs_fh3, data);

/// NFS version 3 time structure
/// Used for file timestamps (access, modify, change)
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug, Default)]
pub struct nfstime3 {
    /// Seconds since Unix epoch (January 1, 1970)
    pub seconds: u32,
    /// Nanoseconds (0-999999999)
    pub nseconds: u32,
}
DeserializeStruct!(nfstime3, seconds, nseconds);
SerializeStruct!(nfstime3, seconds, nseconds);

impl From<nfstime3> for filetime::FileTime {
    fn from(time: nfstime3) -> Self {
        filetime::FileTime::from_unix_time(time.seconds as i64, time.nseconds)
    }
}

/// File attributes in NFS version 3 as defined in RFC 1813 section 2.3.5
/// Contains all the standard attributes associated with a file or directory
/// in the NFS version 3 protocol
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug, Default)]
pub struct fattr3 {
    /// Type of file (regular, directory, symbolic link, etc.)
    pub ftype: ftype3,
    /// File access mode bits. Contains the standard Unix file
    /// permissions and file type information
    pub mode: mode3,
    /// Number of hard links to the file. Indicates how many
    /// directory entries reference this file
    pub nlink: u32,
    /// User ID of the file owner
    pub uid: uid3,
    /// Group ID of the file's group
    pub gid: gid3,
    /// File size in bytes. For regular files, this is the size
    /// of the file data. For directories, this value is implementation-dependent
    pub size: size3,
    /// Size in bytes actually allocated to the file on the server's file system
    /// This may be different from size due to block allocation policies
    pub used: size3,
    /// Device ID information for character or block special files
    /// Contains major and minor numbers for the device
    pub rdev: specdata3,
    /// File system identifier. Uniquely identifies the file system
    /// containing the file
    pub fsid: u64,
    /// File identifier (inode number). Uniquely identifies the file
    /// within its file system
    pub fileid: fileid3,
    /// Time of last access to the file data
    pub atime: nfstime3,
    /// Time of last modification to the file data
    pub mtime: nfstime3,
    /// Time of last status change (modification to the file's attributes)
    pub ctime: nfstime3,
}
DeserializeStruct!(
    fattr3, ftype, mode, nlink, uid, gid, size, used, rdev, fsid, fileid, atime, mtime, ctime
);
SerializeStruct!(
    fattr3, ftype, mode, nlink, uid, gid, size, used, rdev, fsid, fileid, atime, mtime, ctime
);

/// Attributes used in weak cache consistency checking as defined in RFC 1813 section 2.3.8
/// These attributes are used to detect changes to a file by comparing
/// values before and after operations
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug, Default)]
pub struct wcc_attr {
    /// File size in bytes
    pub size: size3,
    /// Last modification time of the file
    pub mtime: nfstime3,
    /// Last status change time of the file
    pub ctime: nfstime3,
}
DeserializeStruct!(wcc_attr, size, mtime, ctime);
SerializeStruct!(wcc_attr, size, mtime, ctime);

/// Pre-operation attributes for weak cache consistency as defined in RFC 1813 section 2.3.8
/// These attributes represent the file state before an operation was performed
/// Used together with post-operation attributes to determine if file state changed
pub type pre_op_attr = Option<wcc_attr>;

/// Post-operation attributes for file information as defined in RFC 1813 section 2.3.8
/// These attributes represent the file state after an operation was performed
/// Returned in almost all NFS procedure responses to allow clients to maintain
/// a consistent cache of file attributes
pub type post_op_attr = Option<fattr3>;

/// Weak cache consistency data as defined in RFC 1813 section 2.3.8
/// Contains file attributes before and after an operation
/// This data structure is returned by operations that modify file attributes
/// to allow clients to update their cached attributes appropriately
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug, Default)]
pub struct wcc_data {
    /// File attributes before operation
    pub before: pre_op_attr,
    /// File attributes after operation
    pub after: post_op_attr,
}
DeserializeStruct!(wcc_data, before, after);
SerializeStruct!(wcc_data, before, after);

pub type post_op_fh3 = Option<nfs_fh3>;
pub type set_mode3 = Option<mode3>;
pub type set_uid3 = Option<uid3>;
pub type set_gid3 = Option<gid3>;
pub type set_size3 = Option<size3>;

/// Specifies how to modify the last access time (atime) during a SETATTR operation.
/// This enum allows the client to either:
/// - Leave the atime unchanged (DONT_CHANGE)
/// - Set it to the server's current time (SET_TO_SERVER_TIME)
/// - Set it to a specific client-provided time (SET_TO_CLIENT_TIME)
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug)]
#[repr(u32)]
pub enum set_atime {
    /// Don't modify the file's last access time
    DONT_CHANGE,
    /// Set the file's last access time to the server's current time
    SET_TO_SERVER_TIME,
    /// Set the file's last access time to the specified time value
    SET_TO_CLIENT_TIME(nfstime3),
}

impl Serialize for set_atime {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        match self {
            set_atime::DONT_CHANGE => {
                0_u32.serialize(dest)?;
            }
            set_atime::SET_TO_SERVER_TIME => {
                1_u32.serialize(dest)?;
            }
            set_atime::SET_TO_CLIENT_TIME(v) => {
                2_u32.serialize(dest)?;
                v.serialize(dest)?;
            }
        }

        Ok(())
    }
}
impl Deserialize for set_atime {
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        match deserialize::<u32>(src)? {
            0 => {
                *self = set_atime::DONT_CHANGE;
            }
            1 => {
                *self = set_atime::SET_TO_SERVER_TIME;
            }
            2 => {
                *self = set_atime::SET_TO_CLIENT_TIME(deserialize(src)?);
            }
            c => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Invalid set_atime value: {c}"),
                ));
            }
        }

        Ok(())
    }
}

/// Specifies how to modify the last modification time (mtime) during a SETATTR operation.
/// This enum allows the client to either:
/// - Leave the mtime unchanged
/// - Set it to the server's current time
/// - Set it to a specific client-provided time
///
/// The discriminant value follows the time_how enumeration from RFC 1813
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug)]
#[repr(u32)]
pub enum set_mtime {
    /// Keep the current modification time unchanged
    DONT_CHANGE,
    /// Update the modification time to the server's current time
    SET_TO_SERVER_TIME,
    /// Set the modification time to a specific timestamp provided by the client
    SET_TO_CLIENT_TIME(nfstime3),
}

impl Serialize for set_mtime {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        match self {
            set_mtime::DONT_CHANGE => {
                0_u32.serialize(dest)?;
            }
            set_mtime::SET_TO_SERVER_TIME => {
                1_u32.serialize(dest)?;
            }
            set_mtime::SET_TO_CLIENT_TIME(v) => {
                2_u32.serialize(dest)?;
                v.serialize(dest)?;
            }
        }

        Ok(())
    }
}
impl Deserialize for set_mtime {
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        match deserialize::<u32>(src)? {
            0 => {
                *self = set_mtime::DONT_CHANGE;
            }
            1 => {
                *self = set_mtime::SET_TO_SERVER_TIME;
            }
            2 => {
                *self = set_mtime::SET_TO_CLIENT_TIME(deserialize(src)?);
            }
            c => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Invalid set_mtime value: {c}"),
                ));
            }
        }

        Ok(())
    }
}

/// Set of file attributes to change in SETATTR operations
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug)]
pub struct sattr3 {
    /// File mode (permissions)
    pub mode: set_mode3,
    /// User ID of owner
    pub uid: set_uid3,
    /// Group ID of owner
    pub gid: set_gid3,
    /// File size
    pub size: set_size3,
    /// Last access time
    pub atime: set_atime,
    /// Last modification time
    pub mtime: set_mtime,
}
DeserializeStruct!(sattr3, mode, uid, gid, size, atime, mtime);
SerializeStruct!(sattr3, mode, uid, gid, size, atime, mtime);

impl Default for sattr3 {
    fn default() -> sattr3 {
        sattr3 {
            mode: set_mode3::None,
            uid: set_uid3::None,
            gid: set_gid3::None,
            size: set_size3::None,
            atime: set_atime::DONT_CHANGE,
            mtime: set_mtime::DONT_CHANGE,
        }
    }
}

/// Arguments for directory operations (specifying directory handle and name)
#[allow(non_camel_case_types)]
#[derive(Clone, Debug, Default)]
pub struct diropargs3 {
    /// Directory file handle
    pub dir: nfs_fh3,
    /// Name within the directory
    pub name: filename3,
}
DeserializeStruct!(diropargs3, dir, name);
SerializeStruct!(diropargs3, dir, name);

/// Data for creating a symbolic link
#[allow(non_camel_case_types)]
#[derive(Debug, Default)]
pub struct symlinkdata3 {
    /// Attributes for the symbolic link
    pub symlink_attributes: sattr3,
    /// Target path for the symbolic link
    pub symlink_data: nfspath3,
}
DeserializeStruct!(symlinkdata3, symlink_attributes, symlink_data);
SerializeStruct!(symlinkdata3, symlink_attributes, symlink_data);

/// Gets the root file handle for mounting
pub fn get_root_mount_handle() -> Vec<u8> {
    vec![0]
}

/// Access permission to read file data or read a directory as defined in RFC 1813 section 3.3.4
pub const ACCESS3_READ: u32 = 0x0001;
/// Access permission to look up names in a directory as defined in RFC 1813 section 3.3.4
pub const ACCESS3_LOOKUP: u32 = 0x0002;
/// Access permission to modify the contents of an existing file as defined in RFC 1813 section 3.3.4
pub const ACCESS3_MODIFY: u32 = 0x0004;
/// Access permission to grow the file's size or extend a directory by adding entries
/// as defined in RFC 1813 section 3.3.4
pub const ACCESS3_EXTEND: u32 = 0x0008;
/// Access permission to delete a file or directory entry as defined in RFC 1813 section 3.3.4
pub const ACCESS3_DELETE: u32 = 0x0010;
/// Access permission to execute a file or traverse a directory as defined in RFC 1813 section 3.3.4
pub const ACCESS3_EXECUTE: u32 = 0x0020;

/// File creation modes for CREATE operations
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug, Default, FromPrimitive, ToPrimitive)]
#[repr(u32)]
pub enum createmode3 {
    /// Normal file creation - doesn't error if file exists
    #[default]
    UNCHECKED = 0,
    /// Return error if file exists
    GUARDED = 1,
    /// Use exclusive create mechanism (with verifier)
    EXCLUSIVE = 2,
}
impl SerializeEnum for createmode3 {}
impl DeserializeEnum for createmode3 {}

pub type sattrguard3 = Option<nfstime3>;

/// Arguments for SETATTR operations
#[allow(non_camel_case_types)]
#[derive(Clone, Debug, Default)]
pub struct SETATTR3args {
    /// File handle for target file
    pub object: nfs_fh3,
    /// New attributes to set
    pub new_attribute: sattr3,
    /// Guard condition for atomic change
    pub guard: Option<nfstime3>,
}
DeserializeStruct!(SETATTR3args, object, new_attribute, guard);
SerializeStruct!(SETATTR3args, object, new_attribute, guard);
