use std::{io, path::Path};

use fernfs::xdr::nfs3::nfsstat3;

/// Result type for NFS operations
pub type NFSResult<T> = Result<T, nfsstat3>;

/// Extension trait for Result to convert IO errors to NFS errors
pub trait ResultExt<T> {
    /// Convert an IO error to an NFS error
    fn or_nfs_error(self) -> NFSResult<T>;
}

impl<T> ResultExt<T> for Result<T, io::Error> {
    fn or_nfs_error(self) -> NFSResult<T> {
        self.map_err(|_| nfsstat3::NFS3ERR_IO)
    }
}

/// Extension trait for Option to convert to NFS errors
pub trait OptionExt<T> {
    /// Convert an Option to an NFS Result
    fn ok_or_nfs_error(self, error: nfsstat3) -> NFSResult<T>;
}

impl<T> OptionExt<T> for Option<T> {
    fn ok_or_nfs_error(self, error: nfsstat3) -> NFSResult<T> {
        self.ok_or(error)
    }
}

/// Enum for refresh results
pub enum RefreshResult {
    /// The fileid was deleted
    Delete,
    /// The fileid needs to be reloaded. mtime has been updated, caches
    /// need to be evicted.
    Reload,
    /// Nothing has changed
    Noop,
}

/// Helper function to check if a path exists without traversing symlinks
pub fn exists_no_traverse(path: &Path) -> bool {
    if let Ok(metadata) = std::fs::symlink_metadata(path) {
        !metadata.is_symlink()
    } else {
        false
    }
}
