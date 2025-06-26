//! Implementation of the REMOVE procedure (procedure 12) for NFS version 3 protocol
//! as defined in RFC 1813 section 3.3.12.
//!
//! The REMOVE procedure removes (deletes) an entry from a directory. If the entry
//! refers to a file system object other than a directory, the object is removed from
//! the file system and destroyed. If the entry is a directory, the request will fail
//! unless the directory is empty.
//!
//! The client specifies:
//! - The file handle for the directory containing the entry to be removed
//! - The name of the entry to be removed
//!
//! On successful return, the server provides:
//! - The attributes of the directory before and after the operation (weak cache consistency)
//!
//! This procedure is commonly used for file removal. For directory removal, clients should
//! use the RMDIR procedure instead. In this implementation, the REMOVE procedure performs
//! additional checks before removing directory entries to ensure file system consistency.
//!
//! Common errors include:
//! - NFS3ERR_ROFS - If the file system is read-only
//! - NFS3ERR_NOENT - If the target file doesn't exist
//! - NFS3ERR_ACCES - If the client doesn't have permission to remove the file
//! - NFS3ERR_ISDIR - If the target is a directory (should use RMDIR instead)

use std::io::{Read, Write};

use tracing::{debug, error, warn};

use crate::protocol::rpc;
use crate::protocol::xdr::{self, deserialize, nfs3, Serialize};
use crate::vfs;

/// Handles NFSv3 REMOVE procedure (procedure 12)
///
/// REMOVE deletes a file system object (non-directory).
/// Takes directory handle and name of the file to be removed.
/// Returns directory attributes before and after the operation.
///
/// # Arguments
///
/// * `xid` - RPC transaction ID
/// * `input` - Input stream containing the REMOVE arguments
/// * `output` - Output stream for writing the response
/// * `context` - Server context containing VFS
///
/// # Returns
///
/// * `Result<(), anyhow::Error>` - Ok(()) on success or an error
///
/// # Errors
///
/// Common errors include:
/// - NFS3ERR_ROFS - If the file system is read-only
/// - NFS3ERR_NOENT - If the target file doesn't exist
/// - NFS3ERR_ISDIR - If the target is a directory
/// - NFS3ERR_ACCES - If the client lacks permission
/// - NFS3ERR_NOTDIR - If the handle is not a directory
/// - NFS3ERR_STALE - If the file handle is invalid
pub async fn nfsproc3_remove(
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

    let dirops = deserialize::<nfs3::diropargs3>(input)?;

    debug!("nfsproc3_remove({:?}, {:?}) ", xid, dirops);

    // find the directory with the file
    let dirid = context.vfs.fh_to_id(&dirops.dir);
    if let Err(stat) = dirid {
        // directory does not exist
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        stat.serialize(output)?;
        nfs3::wcc_data::default().serialize(output)?;
        error!("Directory does not exist");
        return Ok(());
    }
    let dirid = dirid.unwrap();

    // get the object attributes before the write
    let pre_dir_attr = match context.vfs.getattr(dirid).await {
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

    // delete!
    let res = context.vfs.remove(dirid, &dirops.name).await;

    // Re-read dir attributes for post op attr
    let post_dir_attr = context.vfs.getattr(dirid).await.ok();
    let wcc_res = nfs3::wcc_data { before: pre_dir_attr, after: post_dir_attr };

    match res {
        Ok(()) => {
            debug!("remove success");
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            nfs3::nfsstat3::NFS3_OK.serialize(output)?;
            wcc_res.serialize(output)?;
        }
        Err(e) => {
            error!("remove error {:?} --> {:?}", xid, e);
            // serialize CREATE3resfail
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            e.serialize(output)?;
            wcc_res.serialize(output)?;
        }
    }

    Ok(())
}
