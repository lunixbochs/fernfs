//! File System Utilities module provides helper functions for working with the local
//! file system in an NFS-compatible way.
//!
//! This module contains functions for:
//! - Converting between local file system metadata and NFS attributes
//! - Safely checking file existence without traversing symlinks
//! - Setting file attributes based on NFS SETATTR operations
//! - Comparing file metadata for change detection

use std::fs::Metadata;
use std::fs::Permissions;

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

use tokio::fs::OpenOptions;
use tracing::debug;

use crate::protocol::xdr::nfs3;

/// Compares if file metadata has changed in a significant way
///
/// This function checks relevant metadata fields to determine if a file has been
/// modified between two points in time.
///
/// # Arguments
///
/// * `lhs` - First metadata snapshot
/// * `rhs` - Second metadata snapshot
///
/// # Returns
///
/// `true` if the file has changed in a significant way, `false` otherwise
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub fn metadata_differ(lhs: &Metadata, rhs: &Metadata) -> bool {
    lhs.ino() != rhs.ino()
        || lhs.mtime() != rhs.mtime()
        || lhs.len() != rhs.len()
        || lhs.file_type() != rhs.file_type()
}

/// Compares if two NFS file attributes differ in a significant way
///
/// This function checks relevant NFS attribute fields to determine if a file has been
/// modified between two points in time.
///
/// # Arguments
///
/// * `lhs` - First attributes snapshot
/// * `rhs` - Second attributes snapshot
///
/// # Returns
///
/// `true` if the file attributes differ significantly, `false` otherwise
pub fn fattr3_differ(lhs: &nfs3::fattr3, rhs: &nfs3::fattr3) -> bool {
    lhs.fileid != rhs.fileid
        || lhs.mtime.seconds != rhs.mtime.seconds
        || lhs.mtime.nseconds != rhs.mtime.nseconds
        || lhs.size != rhs.size
        || lhs.ftype as u32 != rhs.ftype as u32
}

/// Checks if a path exists without traversing symlinks
///
/// This is a safer alternative to `path.exists()` which can cause deadlocks
/// when encountering recursive symlinks.
///
/// # Arguments
///
/// * `path` - The path to check
///
/// # Returns
///
/// `true` if the path exists (as a file, directory, or symlink), `false` otherwise
pub fn exists_no_traverse(path: &Path) -> bool {
    path.symlink_metadata().is_ok()
}

/// Unmasks file mode bits to ensure writability
///
/// This function ensures that files can be written to by setting the write bit,
/// and also ensures only the relevant permission bits (0o777) are used.
///
/// # Arguments
///
/// * `mode` - Original file mode
///
/// # Returns
///
/// Modified file mode with appropriate permissions
fn mode_unmask(mode: u32) -> u32 {
    // it is possible to create a file we cannot write to.
    // we force writable always.
    let mode = mode | 0x80;
    let mode = Permissions::from_mode(mode);
    mode.mode() & 0x1FF
}

/// Converts filesystem metadata to NFS file attributes
///
/// This function translates local file system metadata into the NFS attributes format,
/// handling different file types appropriately.
///
/// # Arguments
///
/// * `fid` - NFS file ID to use for the file
/// * `meta` - Filesystem metadata to convert
///
/// # Returns
///
/// NFS file attributes structure
pub fn metadata_to_fattr3(fid: nfs3::fileid3, meta: &Metadata) -> nfs3::fattr3 {
    let size = meta.size();
    let file_mode = mode_unmask(meta.mode());
    if meta.is_file() {
        nfs3::fattr3 {
            ftype: nfs3::ftype3::NF3REG,
            mode: file_mode,
            nlink: 1,
            uid: meta.uid(),
            gid: meta.gid(),
            size,
            used: size,
            rdev: nfs3::specdata3::default(),
            fsid: 0,
            fileid: fid,
            atime: nfs3::nfstime3 {
                seconds: meta.atime() as u32,
                nseconds: meta.atime_nsec() as u32,
            },
            mtime: nfs3::nfstime3 {
                seconds: meta.mtime() as u32,
                nseconds: meta.mtime_nsec() as u32,
            },
            ctime: nfs3::nfstime3 {
                seconds: meta.ctime() as u32,
                nseconds: meta.ctime_nsec() as u32,
            },
        }
    } else if meta.is_symlink() {
        nfs3::fattr3 {
            ftype: nfs3::ftype3::NF3LNK,
            mode: file_mode,
            nlink: 1,
            uid: meta.uid(),
            gid: meta.gid(),
            size,
            used: size,
            rdev: nfs3::specdata3::default(),
            fsid: 0,
            fileid: fid,
            atime: nfs3::nfstime3 {
                seconds: meta.atime() as u32,
                nseconds: meta.atime_nsec() as u32,
            },
            mtime: nfs3::nfstime3 {
                seconds: meta.mtime() as u32,
                nseconds: meta.mtime_nsec() as u32,
            },
            ctime: nfs3::nfstime3 {
                seconds: meta.ctime() as u32,
                nseconds: meta.ctime_nsec() as u32,
            },
        }
    } else {
        nfs3::fattr3 {
            ftype: nfs3::ftype3::NF3DIR,
            mode: file_mode,
            nlink: 2,
            uid: meta.uid(),
            gid: meta.gid(),
            size,
            used: size,
            rdev: nfs3::specdata3::default(),
            fsid: 0,
            fileid: fid,
            atime: nfs3::nfstime3 {
                seconds: meta.atime() as u32,
                nseconds: meta.atime_nsec() as u32,
            },
            mtime: nfs3::nfstime3 {
                seconds: meta.mtime() as u32,
                nseconds: meta.mtime_nsec() as u32,
            },
            ctime: nfs3::nfstime3 {
                seconds: meta.ctime() as u32,
                nseconds: meta.ctime_nsec() as u32,
            },
        }
    }
}

/// Sets attributes of a file path based on NFS SETATTR operation
///
/// This function applies the attributes specified in an NFS SETATTR request
/// to a file or directory specified by path.
///
/// # Arguments
///
/// * `path` - Path to the file or directory
/// * `setattr` - NFS attributes to set
///
/// # Returns
///
/// Result indicating success or NFS error code
pub async fn path_setattr(path: &Path, setattr: &nfs3::sattr3) -> Result<(), nfs3::nfsstat3> {
    match setattr.atime {
        nfs3::set_atime::SET_TO_SERVER_TIME => {
            let _ = filetime::set_file_atime(path, filetime::FileTime::now());
        }
        nfs3::set_atime::SET_TO_CLIENT_TIME(time) => {
            let _ = filetime::set_file_atime(path, time.into());
        }
        _ => {}
    };
    match setattr.mtime {
        nfs3::set_mtime::SET_TO_SERVER_TIME => {
            let _ = filetime::set_file_mtime(path, filetime::FileTime::now());
        }
        nfs3::set_mtime::SET_TO_CLIENT_TIME(time) => {
            let _ = filetime::set_file_mtime(path, time.into());
        }
        _ => {}
    };
    if let nfs3::set_mode3::Some(mode) = setattr.mode {
        debug!(" -- set permissions {:?} {:?}", path, mode);
        let mode = mode_unmask(mode);
        let _ = std::fs::set_permissions(path, Permissions::from_mode(mode));
    };
    if setattr.uid.is_some() {
        debug!("Set uid not implemented");
    }
    if setattr.gid.is_some() {
        debug!("Set gid not implemented");
    }
    if let nfs3::set_size3::Some(size3) = setattr.size {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .truncate(false)
            .open(path)
            .await
            .or(Err(nfs3::nfsstat3::NFS3ERR_IO))?;
        debug!(" -- set size {:?} {:?}", path, size3);
        file.set_len(size3).await.or(Err(nfs3::nfsstat3::NFS3ERR_IO))?;
    }
    Ok(())
}

/// Sets attributes of an open file based on NFS SETATTR operation
///
/// This function applies the attributes specified in an NFS SETATTR request
/// to an already open file handle.
///
/// # Arguments
///
/// * `file` - Open file handle
/// * `setattr` - NFS attributes to set
///
/// # Returns
///
/// Result indicating success or NFS error code
pub async fn file_setattr(
    file: &std::fs::File,
    setattr: &nfs3::sattr3,
) -> Result<(), nfs3::nfsstat3> {
    if let nfs3::set_mode3::Some(mode) = setattr.mode {
        debug!(" -- set permissions {:?}", mode);
        let mode = mode_unmask(mode);
        let _ = file.set_permissions(Permissions::from_mode(mode));
    }
    if let nfs3::set_size3::Some(size3) = setattr.size {
        debug!(" -- set size {:?}", size3);
        file.set_len(size3).or(Err(nfs3::nfsstat3::NFS3ERR_IO))?;
    }
    Ok(())
}
