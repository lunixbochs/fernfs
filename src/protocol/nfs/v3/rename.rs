//! Implementation of the RENAME procedure (procedure 14) for NFS version 3 protocol
//! as defined in RFC 1813 section 3.3.14.
//!
//! The RENAME procedure changes the name of a file system object in the directory
//! hierarchy. The operation renames a file or directory, possibly by moving it to
//! a different directory.
//!
//! The client specifies:
//! - The file handle and filename identifying the source object
//! - The file handle and filename identifying the target location
//!
//! On successful return, the server provides:
//! - The attributes of the source directory before and after the operation
//! - The attributes of the target directory before and after the operation
//!
//! If the source and target directories are the same, the attributes returned for
//! both are identical. If the target object already exists, it is removed as part
//! of the renaming operation.
//!
//! Common errors:
//! - NFS3ERR_NOENT: Source file/directory doesn't exist
//! - NFS3ERR_ACCES: Permission denied
//! - NFS3ERR_XDEV: Attempt to move between file systems
//! - NFS3ERR_ROFS: Write attempted on read-only file system
//! - NFS3ERR_NOTDIR: A component of path prefix is not a directory

use std::io::{Read, Write};

use tracing::{debug, error, warn};

use crate::protocol::rpc;
use crate::protocol::xdr::{self, deserialize, nfs3, Serialize};
use crate::vfs;

/// Handles NFSv3 RENAME procedure (procedure 14)
///
/// RENAME changes the name of a file system object.
/// Takes source directory handle, source name, target directory handle, and target name.
/// Returns attributes of both source and target directories before and after the operation.
///
/// This procedure implements atomic rename semantics - either the operation
/// completes entirely or not at all. If a target object already exists, it is
/// first removed and then the source object is renamed.
///
/// # Arguments
///
/// * `xid` - RPC transaction ID
/// * `input` - Input stream containing the RENAME arguments
/// * `output` - Output stream for writing the response
/// * `context` - Server context containing VFS
///
/// # Returns
///
/// * `Result<(), anyhow::Error>` - Ok(()) on success or an error
pub async fn nfsproc3_rename(
    xid: u32,
    input: &mut impl Read,
    output: &mut impl Write,
    context: &rpc::Context,
) -> Result<(), anyhow::Error> {
    // if we do not have write capabilities
    if !matches!(context.vfs.capabilities(), vfs::Capabilities::ReadWrite) {
        warn!("No write capabilities.");
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        nfs3::nfsstat3::NFS3ERR_ROFS.serialize(output)?;
        nfs3::wcc_data::default().serialize(output)?;
        return Ok(());
    }

    let fromdirops = deserialize::<nfs3::diropargs3>(input)?;
    let todirops = deserialize::<nfs3::diropargs3>(input)?;

    debug!("nfsproc3_rename({:?}, {:?}, {:?}) ", xid, fromdirops, todirops);

    // find the from directory
    let from_dirid = context.vfs.fh_to_id(&fromdirops.dir);
    if let Err(stat) = from_dirid {
        // directory does not exist
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        stat.serialize(output)?;
        nfs3::wcc_data::default().serialize(output)?;
        error!("Directory does not exist");
        return Ok(());
    }

    // find the to directory
    let to_dirid = context.vfs.fh_to_id(&todirops.dir);
    if let Err(stat) = to_dirid {
        // directory does not exist
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        stat.serialize(output)?;
        nfs3::wcc_data::default().serialize(output)?;
        error!("Directory does not exist");
        return Ok(());
    }

    // found the directory, get the attributes
    let from_dirid = from_dirid.unwrap();
    let to_dirid = to_dirid.unwrap();

    // get the object attributes before the write
    let pre_from_dir_attr = match context.vfs.getattr(from_dirid).await {
        Ok(v) => {
            let wccattr = nfs3::wcc_attr { size: v.size, mtime: v.mtime, ctime: v.ctime };
            nfs3::pre_op_attr::Some(wccattr)
        }
        Err(stat) => {
            error!("Cannot stat directory");
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            stat.serialize(output)?;
            nfs3::wcc_data::default().serialize(output)?;
            return Ok(());
        }
    };

    // get the object attributes before the write
    let pre_to_dir_attr = match context.vfs.getattr(to_dirid).await {
        Ok(v) => {
            let wccattr = nfs3::wcc_attr { size: v.size, mtime: v.mtime, ctime: v.ctime };
            nfs3::pre_op_attr::Some(wccattr)
        }
        Err(stat) => {
            error!("Cannot stat directory");
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            stat.serialize(output)?;
            nfs3::wcc_data::default().serialize(output)?;
            return Ok(());
        }
    };

    // rename!
    let res = context.vfs.rename(from_dirid, &fromdirops.name, to_dirid, &todirops.name).await;

    // Re-read dir attributes for post op attr
    let post_from_dir_attr = context.vfs.getattr(from_dirid).await.ok();
    let post_to_dir_attr = context.vfs.getattr(to_dirid).await.ok();
    let from_wcc_res = nfs3::wcc_data { before: pre_from_dir_attr, after: post_from_dir_attr };

    let to_wcc_res = nfs3::wcc_data { before: pre_to_dir_attr, after: post_to_dir_attr };

    match res {
        Ok(()) => {
            debug!("rename success");
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            nfs3::nfsstat3::NFS3_OK.serialize(output)?;
            from_wcc_res.serialize(output)?;
            to_wcc_res.serialize(output)?;
        }
        Err(e) => {
            error!("rename error {:?} --> {:?}", xid, e);
            // serialize CREATE3resfail
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            e.serialize(output)?;
            from_wcc_res.serialize(output)?;
            to_wcc_res.serialize(output)?;
        }
    }

    Ok(())
}
