use std::sync::{Arc, RwLock};

use fernfs::xdr::nfs3;

use crate::fs_contents::FSContents;

/// Represents a file system entry in the demo NFS file system.
/// Can be either a file or a directory depending on its contents.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct FSEntry {
    /// Unique identifier for the file system entry
    pub id: nfs3::fileid3,
    /// File attributes containing metadata like type, permissions, size, etc.
    pub attr: nfs3::fattr3,
    /// Name of the file system entry
    pub name: nfs3::filename3,
    /// ID of the parent directory
    pub parent: nfs3::fileid3,
    /// Actual content of the entry (either file data or directory listing)
    pub contents: FSContents,
}

/// Creates a file entry with the specified parameters.
///
/// Returns a fully initialized FSEntry with file type and default attributes.
pub fn make_file(name: &str, id: nfs3::fileid3, parent: nfs3::fileid3, contents: &[u8]) -> FSEntry {
    let attr = nfs3::fattr3 {
        ftype: nfs3::ftype3::NF3REG,
        mode: 0o755,
        nlink: 1,
        uid: 507,
        gid: 507,
        size: contents.len() as u64,
        used: contents.len() as u64,
        rdev: nfs3::specdata3::default(),
        fsid: 0,
        fileid: id,
        atime: nfs3::nfstime3::default(),
        mtime: nfs3::nfstime3::default(),
        ctime: nfs3::nfstime3::default(),
    };
    FSEntry {
        id,
        attr,
        name: name.as_bytes().into(),
        parent,
        contents: FSContents::File(Arc::new(RwLock::new(contents.to_vec()))),
    }
}

/// Creates a directory entry with the specified parameters.
///
/// Returns a fully initialized FSEntry with directory type and default attributes.
pub fn make_dir(
    name: &str,
    id: nfs3::fileid3,
    parent: nfs3::fileid3,
    contents: Vec<nfs3::fileid3>,
) -> FSEntry {
    let attr = nfs3::fattr3 {
        ftype: nfs3::ftype3::NF3DIR,
        mode: 0o777,
        nlink: 1,
        uid: 507,
        gid: 507,
        size: 0,
        used: 0,
        rdev: nfs3::specdata3::default(),
        fsid: 0,
        fileid: id,
        atime: nfs3::nfstime3::default(),
        mtime: nfs3::nfstime3::default(),
        ctime: nfs3::nfstime3::default(),
    };
    FSEntry {
        id,
        attr,
        name: name.as_bytes().into(),
        parent,
        contents: FSContents::Directory(contents),
    }
}
