//! Virtual File System (VFS) module defines the interface between the NFS server
//! and the underlying file system implementations.
//!
//! This module provides:
//! - The core NFSFileSystem trait that must be implemented to create an NFS-exportable file system
//! - Support structures and enumerations for directory entries and file operations
//! - File handle management with generation numbers for stale handle detection
//! - Default implementations for common operations to simplify custom implementations
//!
//! The VFS layer abstracts the file system operations required by NFS v3 protocol (RFC 1813)
//! and allows different storage backends to be used with the server. It translates between
//! the NFS procedure calls defined in the protocol and actual file system operations.
//!
//! Key features of the VFS design include:
//! - Stateless operation, using file identifiers instead of open file handles
//! - Support for all NFSv3 file operations (read, write, create, etc.)
//! - Weak cache consistency through file attributes
//! - Support for both synchronous and asynchronous I/O operations
//! - File handle management that detects stale handles after server restarts

use std::cmp::Ordering;

use async_trait::async_trait;

use crate::protocol::xdr::nfs3;

/// Simplified directory entry containing only file ID and name
///
/// Used for simple directory listing operations where full attributes are not needed
#[derive(Default, Debug)]
pub struct DirEntrySimple {
    /// Unique file identifier within the file system (similar to inode number)
    pub fileid: nfs3::fileid3,
    /// File name (without path components)
    pub name: nfs3::filename3,
}

/// Result returned by readdir_simple operations
///
/// Contains a vector of simplified directory entries and an EOF flag
#[derive(Default, Debug)]
pub struct ReadDirSimpleResult {
    /// List of directory entries with minimal information
    pub entries: Vec<DirEntrySimple>,
    /// Indicates if the end of directory has been reached
    pub end: bool,
}

/// Full directory entry containing file ID, name and attributes
///
/// Used for extended directory listing operations like READDIRPLUS
#[derive(Default, Debug)]
pub struct DirEntry {
    /// Unique file identifier within the file system (similar to inode number)
    pub fileid: nfs3::fileid3,
    /// File name (without path components)
    pub name: nfs3::filename3,
    /// Complete file attributes
    pub attr: nfs3::fattr3,
}

/// Result returned by readdir operations
///
/// Contains a vector of complete directory entries and an EOF flag
#[derive(Default, Debug)]
pub struct ReadDirResult {
    /// List of directory entries with full information
    pub entries: Vec<DirEntry>,
    /// Indicates if the end of directory has been reached
    pub end: bool,
}

impl ReadDirSimpleResult {
    /// Converts a full ReadDirResult to a simplified ReadDirSimpleResult
    ///
    /// This allows implementations to provide just the full readdir operation,
    /// and the simplified version can be derived automatically.
    fn from_readdir_result(result: &ReadDirResult) -> ReadDirSimpleResult {
        let entries: Vec<DirEntrySimple> = result
            .entries
            .iter()
            .map(|e| DirEntrySimple { fileid: e.fileid, name: e.name.clone() })
            .collect();
        ReadDirSimpleResult { entries, end: result.end }
    }
}

/// Defines the access capabilities supported by a file system implementation
pub enum Capabilities {
    /// File system supports read operations only
    ReadOnly,
    /// File system supports both read and write operations
    ReadWrite,
}

/// The basic API to implement to provide an NFS file system
///
/// Opaque FH
/// ---------
/// Files are only uniquely identified by a 64-bit file id. (basically an inode number)
/// We automatically produce internally the opaque filehandle which is comprised of
///  - A 64-bit generation number derived from the server startup time
///   (i.e. so the opaque file handle expires when the NFS server restarts)
///  - The 64-bit file id
//
/// readdir pagination
/// ------------------
/// We do not use cookie verifier. We just use the start_after.  The
/// implementation should allow startat to start at any position. That is,
/// the next query to readdir may be the last entry in the previous readdir
/// response.
//
/// There is a wierd annoying thing about readdir that limits the number
/// of bytes in the response (instead of the number of entries). The caller
/// will have to truncate the readdir response / issue more calls to readdir
/// accordingly to fill up the expected number of bytes without exceeding it.
//
/// Other requirements
/// ------------------
///  getattr needs to be fast. NFS uses that a lot
//
///  The 0 fileid is reserved and should not be used
///
#[async_trait]
pub trait NFSFileSystem: Sync {
    /// Gets the server generation number, initializing it on first call
    ///
    /// The generation number is based on the server startup time and is used to detect
    /// stale file handles from previous server instances.
    fn generation(&self) -> u64;

    /// Returns the set of capabilities supported by this file system implementation
    ///
    /// This determines whether write operations are allowed on the file system.
    fn capabilities(&self) -> Capabilities;

    /// Returns the file ID of the root directory "/"
    ///
    /// This ID is used as the starting point for all path lookups and is typically
    /// the first file handle requested by NFS clients during mount operations.
    fn root_dir(&self) -> nfs3::fileid3;

    /// Look up the ID of a file or directory within a parent directory
    ///
    /// This method translates a file name to a file ID within the context of a directory.
    /// For example, given a directory "dir/" containing a file "a.txt", a call to
    /// lookup(id_of("dir/"), "a.txt") should return the ID of the file "dir/a.txt".
    ///
    /// # Arguments
    /// * `dirid` - The file ID of the parent directory
    /// * `filename` - The name of the file or directory to look up
    ///
    /// # Returns
    /// * `Result<fileid3, nfsstat3>` - The file ID on success, or an NFS error code
    async fn lookup(
        &self,
        dirid: nfs3::fileid3,
        filename: &nfs3::filename3,
    ) -> Result<nfs3::fileid3, nfs3::nfsstat3>;

    /// Returns the attributes of a file or directory
    ///
    /// This method retrieves the complete set of file attributes for the specified file ID.
    /// It should be optimized for performance as it is called frequently by NFS clients.
    ///
    /// # Arguments
    /// * `id` - The file ID to get attributes for
    ///
    /// # Returns
    /// * `Result<fattr3, nfsstat3>` - The file attributes on success, or an NFS error code
    async fn getattr(&self, id: nfs3::fileid3) -> Result<nfs3::fattr3, nfs3::nfsstat3>;

    /// Sets the attributes of a file or directory
    ///
    /// This method allows changing file metadata such as permissions, ownership, and timestamps.
    /// Read-only file systems should return NFS3ERR_ROFS.
    ///
    /// # Arguments
    /// * `id` - The file ID to set attributes for
    /// * `setattr` - The attributes to set
    ///
    /// # Returns
    /// * `Result<fattr3, nfsstat3>` - The updated file attributes on success, or an NFS error code
    async fn setattr(
        &self,
        id: nfs3::fileid3,
        setattr: nfs3::sattr3,
    ) -> Result<nfs3::fattr3, nfs3::nfsstat3>;

    /// Reads data from a file
    ///
    /// This method reads a portion of a file's content starting at the specified offset.
    /// If offset+count extends beyond the end of the file, all remaining data should be returned.
    /// The returned boolean indicates whether the read operation reached the end of the file.
    ///
    /// # Arguments
    /// * `id` - The file ID to read from
    /// * `offset` - Byte offset within the file to start reading
    /// * `count` - Maximum number of bytes to read
    ///
    /// # Returns
    /// * `Result<(Vec<u8>, bool), nfsstat3>` - The read data and EOF flag on success, or an NFS error code
    async fn read(
        &self,
        id: nfs3::fileid3,
        offset: u64,
        count: u32,
    ) -> Result<(Vec<u8>, bool), nfs3::nfsstat3>;

    /// Writes data to a file
    ///
    /// This method writes data to a file starting at the specified offset.
    /// If the write extends beyond the current file size, the file should be extended.
    /// Read-only file systems should return NFS3ERR_ROFS.
    ///
    /// # Arguments
    /// * `id` - The file ID to write to
    /// * `offset` - Byte offset within the file to start writing
    /// * `data` - The data to write
    ///
    /// # Returns
    /// * `Result<fattr3, nfsstat3>` - The updated file attributes on success, or an NFS error code
    async fn write(
        &self,
        id: nfs3::fileid3,
        offset: u64,
        data: &[u8],
    ) -> Result<nfs3::fattr3, nfs3::nfsstat3>;

    /// Creates a new file with the specified attributes
    ///
    /// This method creates a new file in the specified directory.
    /// Read-only file systems should return NFS3ERR_ROFS.
    ///
    /// # Arguments
    /// * `dirid` - The parent directory ID
    /// * `filename` - The name for the new file
    /// * `attr` - Initial attributes for the new file
    ///
    /// # Returns
    /// * `Result<(fileid3, fattr3), nfsstat3>` - The new file's ID and attributes on success, or an NFS error code
    async fn create(
        &self,
        dirid: nfs3::fileid3,
        filename: &nfs3::filename3,
        attr: nfs3::sattr3,
    ) -> Result<(nfs3::fileid3, nfs3::fattr3), nfs3::nfsstat3>;

    /// Creates a file if it doesn't exist (exclusive creation)
    ///
    /// This method creates a new file only if it doesn't already exist.
    /// Read-only file systems should return NFS3ERR_ROFS.
    ///
    /// # Arguments
    /// * `dirid` - The parent directory ID
    /// * `filename` - The name for the new file
    ///
    /// # Returns
    /// * `Result<fileid3, nfsstat3>` - The new file's ID on success, or an NFS error code
    async fn create_exclusive(
        &self,
        dirid: nfs3::fileid3,
        filename: &nfs3::filename3,
    ) -> Result<nfs3::fileid3, nfs3::nfsstat3>;

    /// Creates a new directory
    ///
    /// This method creates a new directory in the specified parent directory.
    /// Read-only file systems should return NFS3ERR_ROFS.
    ///
    /// # Arguments
    /// * `dirid` - The parent directory ID
    /// * `dirname` - The name for the new directory
    ///
    /// # Returns
    /// * `Result<(fileid3, fattr3), nfsstat3>` - The new directory's ID and attributes on success, or an NFS error code
    async fn mkdir(
        &self,
        dirid: nfs3::fileid3,
        dirname: &nfs3::filename3,
    ) -> Result<(nfs3::fileid3, nfs3::fattr3), nfs3::nfsstat3>;

    /// Removes a file or empty directory
    ///
    /// This method deletes a file or empty directory from the specified parent directory.
    /// Read-only file systems should return NFS3ERR_ROFS.
    ///
    /// # Arguments
    /// * `dirid` - The parent directory ID
    /// * `filename` - The name of the file or directory to remove
    ///
    /// # Returns
    /// * `Result<(), nfsstat3>` - Success or an NFS error code
    async fn remove(
        &self,
        dirid: nfs3::fileid3,
        filename: &nfs3::filename3,
    ) -> Result<(), nfs3::nfsstat3>;

    /// Renames a file or directory
    ///
    /// This method renames and/or moves a file or directory.
    /// Read-only file systems should return NFS3ERR_ROFS.
    ///
    /// # Arguments
    /// * `from_dirid` - The source parent directory ID
    /// * `from_filename` - The source file or directory name
    /// * `to_dirid` - The destination parent directory ID
    /// * `to_filename` - The destination file or directory name
    ///
    /// # Returns
    /// * `Result<(), nfsstat3>` - Success or an NFS error code
    async fn rename(
        &self,
        from_dirid: nfs3::fileid3,
        from_filename: &nfs3::filename3,
        to_dirid: nfs3::fileid3,
        to_filename: &nfs3::filename3,
    ) -> Result<(), nfs3::nfsstat3>;

    /// Reads directory entries with pagination support
    ///
    /// This method retrieves a list of entries from a directory, starting after a specific entry.
    /// The directory listing should be deterministic and support resuming from any point.
    ///
    /// # Arguments
    /// * `dirid` - The directory ID to read
    /// * `start_after` - The file ID after which to start listing (0 means start from beginning)
    /// * `max_entries` - Maximum number of entries to return
    ///
    /// # Returns
    /// * `Result<ReadDirResult, nfsstat3>` - Directory entries and EOF flag on success, or an NFS error code
    async fn readdir(
        &self,
        dirid: nfs3::fileid3,
        start_after: nfs3::fileid3,
        max_entries: usize,
    ) -> Result<ReadDirResult, nfs3::nfsstat3>;

    /// Simplified version of readdir that returns only file names and IDs
    ///
    /// This is a convenience method that provides a simpler interface when full
    /// file attributes are not needed. The default implementation calls the full
    /// readdir method and converts the result.
    ///
    /// # Arguments
    /// * `dirid` - The directory ID to read
    /// * `count` - Maximum number of entries to return
    ///
    /// # Returns
    /// * `Result<ReadDirSimpleResult, nfsstat3>` - Simplified directory entries on success, or an NFS error code
    async fn readdir_simple(
        &self,
        dirid: nfs3::fileid3,
        count: usize,
    ) -> Result<ReadDirSimpleResult, nfs3::nfsstat3> {
        Ok(ReadDirSimpleResult::from_readdir_result(&self.readdir(dirid, 0, count).await?))
    }

    /// Creates a symbolic link
    ///
    /// This method creates a symbolic link in the specified directory pointing to the target path.
    /// Read-only file systems should return NFS3ERR_ROFS.
    ///
    /// # Arguments
    /// * `dirid` - The parent directory ID
    /// * `linkname` - The name of the symbolic link
    /// * `symlink` - The target path that the link points to
    /// * `attr` - Initial attributes for the symbolic link
    ///
    /// # Returns
    /// * `Result<(fileid3, fattr3), nfsstat3>` - The new symlink's ID and attributes on success, or an NFS error code
    async fn symlink(
        &self,
        dirid: nfs3::fileid3,
        linkname: &nfs3::filename3,
        symlink: &nfs3::nfspath3,
        attr: &nfs3::sattr3,
    ) -> Result<(nfs3::fileid3, nfs3::fattr3), nfs3::nfsstat3>;

    /// Reads the target of a symbolic link
    ///
    /// This method retrieves the target path that a symbolic link points to.
    ///
    /// # Arguments
    /// * `id` - The file ID of the symbolic link
    ///
    /// # Returns
    /// * `Result<nfspath3, nfsstat3>` - The target path on success, or an NFS error code
    async fn readlink(&self, id: nfs3::fileid3) -> Result<nfs3::nfspath3, nfs3::nfsstat3>;

    /// Creates a hard link
    ///
    /// This method creates a new name (hard link) for an existing file.
    /// Read-only file systems should return NFS3ERR_ROFS.
    ///
    /// # Arguments
    /// * `file_id` - The ID of the existing file to link to
    /// * `link_dir_id` - The parent directory ID for the new link
    /// * `link_name` - The name for the new link
    ///
    /// # Returns
    /// * `Result<fattr3, nfsstat3>` - The updated file attributes on success, or an NFS error code
    async fn link(
        &self,
        file_id: nfs3::fileid3,
        link_dir_id: nfs3::fileid3,
        link_name: &nfs3::filename3,
    ) -> Result<nfs3::fattr3, nfs3::nfsstat3>;

    /// Creates a special node (character device, block device, socket, or FIFO)
    ///
    /// This method creates a special device file in the specified directory.
    /// Read-only file systems or implementations that don't support device files
    /// should return NFS3ERR_ROFS or NFS3ERR_NOTSUPP.
    ///
    /// # Arguments
    /// * `dir_id` - The parent directory ID
    /// * `name` - The name for the new special file
    /// * `ftype` - The type of special file to create
    /// * `specdata` - Device-specific information (major/minor numbers)
    /// * `attrs` - Initial attributes for the new file
    ///
    /// # Returns
    /// * `Result<(fileid3, fattr3), nfsstat3>` - The new file's ID and attributes on success, or an NFS error code
    async fn mknod(
        &self,
        dir_id: nfs3::fileid3,
        name: &nfs3::filename3,
        ftype: nfs3::ftype3,
        specdata: nfs3::specdata3,
        attrs: &nfs3::sattr3,
    ) -> Result<(nfs3::fileid3, nfs3::fattr3), nfs3::nfsstat3>;

    /// Commits data written to a file to stable storage
    ///
    /// This method ensures that previously written data is committed to stable storage.
    /// Read-only file systems should return NFS3ERR_ROFS.
    ///
    /// # Arguments
    /// * `file_id` - The file ID to commit
    /// * `offset` - Starting offset for the commit operation
    /// * `count` - Number of bytes to commit
    ///
    /// # Returns
    /// * `Result<fattr3, nfsstat3>` - The file attributes after commit on success, or an NFS error code
    async fn commit(
        &self,
        file_id: nfs3::fileid3,
        offset: u64,
        count: u32,
    ) -> Result<nfs3::fattr3, nfs3::nfsstat3>;

    /// Retrieves static file system information
    ///
    /// This method provides information about the file system's capabilities and parameters.
    /// The default implementation returns a standard set of values suitable for most file systems.
    ///
    /// # Arguments
    /// * `root_fileid` - The file ID of the root directory
    ///
    /// # Returns
    /// * `Result<fsinfo3, nfsstat3>` - File system information on success, or an NFS error code
    async fn fsinfo(
        &self,
        root_fileid: nfs3::fileid3,
    ) -> Result<nfs3::fs::fsinfo3, nfs3::nfsstat3> {
        let dir_attr: nfs3::post_op_attr = self.getattr(root_fileid).await.ok();

        let res = nfs3::fs::fsinfo3 {
            obj_attributes: dir_attr,
            rtmax: 1024 * 1024,
            rtpref: 1024 * 124,
            rtmult: 1024 * 1024,
            wtmax: 1024 * 1024,
            wtpref: 1024 * 1024,
            wtmult: 1024 * 1024,
            dtpref: 1024 * 1024,
            maxfilesize: 128 * 1024 * 1024 * 1024,
            time_delta: nfs3::nfstime3 { seconds: 0, nseconds: 1000000 },
            properties: nfs3::fs::FSF_SYMLINK
                | nfs3::fs::FSF_HOMOGENEOUS
                | nfs3::fs::FSF_CANSETTIME,
        };
        Ok(res)
    }

    /// Converts a file ID to an opaque NFS file handle
    ///
    /// This method creates an opaque file handle from a file ID by combining
    /// the server's generation number with the file ID. The generation number
    /// ensures that file handles from previous server instances can be detected.
    ///
    /// # Arguments
    /// * `id` - The file ID to convert
    ///
    /// # Returns
    /// * `nfs_fh3` - The opaque NFS file handle
    fn id_to_fh(&self, id: nfs3::fileid3) -> nfs3::nfs_fh3 {
        let gennum = self.generation();
        let mut ret: Vec<u8> = Vec::new();
        ret.extend_from_slice(&gennum.to_le_bytes());
        ret.extend_from_slice(&id.to_le_bytes());
        nfs3::nfs_fh3 { data: ret }
    }

    /// Converts an opaque NFS file handle to a file ID
    ///
    /// This method extracts the file ID from an opaque file handle and verifies that
    /// the file handle's generation number matches the current server instance.
    ///
    /// # Arguments
    /// * `id` - The opaque NFS file handle
    ///
    /// # Returns
    /// * `Result<fileid3, nfsstat3>` - The file ID on success, or an NFS error code
    ///   Returns NFS3ERR_STALE if the file handle is from a previous server instance
    ///   Returns NFS3ERR_BADHANDLE if the file handle is malformed
    fn fh_to_id(&self, id: &nfs3::nfs_fh3) -> Result<nfs3::fileid3, nfs3::nfsstat3> {
        if id.data.len() != 16 {
            return Err(nfs3::nfsstat3::NFS3ERR_BADHANDLE);
        }
        let gen = u64::from_le_bytes(id.data[0..8].try_into().unwrap());
        let id = u64::from_le_bytes(id.data[8..16].try_into().unwrap());
        let gennum = self.generation();
        match gen.cmp(&gennum) {
            Ordering::Less => Err(nfs3::nfsstat3::NFS3ERR_STALE),
            Ordering::Greater => Err(nfs3::nfsstat3::NFS3ERR_BADHANDLE),
            Ordering::Equal => Ok(id),
        }
    }

    /// Converts a path to a file ID by walking the directory structure
    ///
    /// This method translates a full path to a file ID by traversing the directory
    /// hierarchy starting from the root directory. The default implementation uses
    /// lookup() to navigate the path components.
    ///
    /// # Arguments
    /// * `path` - The path to convert
    ///
    /// # Returns
    /// * `Result<fileid3, nfsstat3>` - The file ID on success, or an NFS error code
    async fn path_to_id(&self, path: &[u8]) -> Result<nfs3::fileid3, nfs3::nfsstat3> {
        let splits = path.split(|&r| r == b'/');
        let mut fid = self.root_dir();
        for component in splits {
            if component.is_empty() {
                continue;
            }
            fid = self.lookup(fid, &component.into()).await?;
        }
        Ok(fid)
    }

    /// Returns a unique server ID used for cookie verification
    ///
    /// This method provides a value that clients can use to verify that they are
    /// communicating with the same server instance. The default implementation
    /// uses the server's generation number.
    ///
    /// # Returns
    /// * `cookieverf3` - A unique identifier for this server instance
    fn server_id(&self) -> nfs3::cookieverf3 {
        self.generation().to_le_bytes()
    }
}
