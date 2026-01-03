use std::sync::{Arc, RwLock};

use fernfs::xdr::nfs3;

/// Storage representation for file system entries.
/// Used to represent either file data or directory listings.
#[derive(Debug, Clone)]
pub enum FSContents {
    /// Contains link to file data as a byte vector
    File(Arc<RwLock<Vec<u8>>>),
    /// Contains a list of file IDs for directory entries
    Directory(Vec<nfs3::fileid3>),
}
