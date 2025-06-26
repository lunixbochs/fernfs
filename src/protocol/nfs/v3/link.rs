//! Implementation of the LINK procedure (procedure 15) for NFS version 3 protocol
//! as defined in RFC 1813 section 3.3.15.
//!
//! The LINK procedure creates a hard link from one file to another. A hard link
//! is a second directory entry referring to the same file with an identical
//! file system object.
//!
//! The client specifies:
//! - The file handle for the existing file (target)
//! - The directory file handle and name for the new link (where to create the link)
//!
//! On successful return, the server provides:
//! - The file attributes of the target file after the operation
//! - The attributes of the directory before and after the operation (weak cache consistency)
//!
//! Hard links can be created only within a single file system (volume).
//! Servers should return NFS3ERR_XDEV if a cross-device link is attempted.

use std::io::{Read, Write};

use tracing::{debug, warn};

use crate::protocol::rpc;
use crate::protocol::xdr::{self, deserialize, nfs3, Serialize};
use crate::vfs;

/// Handles NFSv3 LINK procedure (procedure 15)
///
/// LINK creates a hard link to an existing file.
/// Takes file handle for target file, directory handle, and name for the new link.
/// Returns file attributes and directory attributes before and after the operation.
///
/// # Arguments
///
/// * `xid` - RPC transaction ID
/// * `input` - Input stream containing the LINK arguments
/// * `output` - Output stream for writing the response
/// * `context` - Server context containing VFS
///
/// # Returns
///
/// * `Result<(), anyhow::Error>` - Ok(()) on success or an error
pub async fn nfsproc3_link(
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
        nfs3::post_op_attr::None.serialize(output)?;
        nfs3::wcc_data::default().serialize(output)?;
        return Ok(());
    }

    let args = deserialize::<nfs3::file::LINK3args>(input)?;
    debug!("nfsproc3_link({:?}, {:?}) ", xid, args);

    // Get the file id
    let fileid = context.vfs.fh_to_id(&args.file);
    if let Err(stat) = fileid {
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        stat.serialize(output)?;
        nfs3::post_op_attr::None.serialize(output)?;
        nfs3::wcc_data::default().serialize(output)?;
        return Ok(());
    }
    let fileid = fileid.unwrap();

    // Get the directory id
    let dirid = context.vfs.fh_to_id(&args.link.dir);
    if let Err(stat) = dirid {
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        stat.serialize(output)?;
        nfs3::post_op_attr::None.serialize(output)?;
        nfs3::wcc_data::default().serialize(output)?;
        return Ok(());
    }
    let dirid = dirid.unwrap();

    // Get the directory attributes before the operation
    let pre_dir_attr = context
        .vfs
        .getattr(dirid)
        .await
        .map(|v| nfs3::wcc_attr { size: v.size, mtime: v.mtime, ctime: v.ctime })
        .ok();

    // Call VFS link method
    match context.vfs.link(fileid, dirid, &args.link.name).await {
        Ok(fattr) => {
            // Get file attributes
            let file_attr = nfs3::post_op_attr::Some(fattr);

            // Get the directory attributes after the operation
            let post_dir_attr = context.vfs.getattr(dirid).await.ok();

            let wcc_res = nfs3::wcc_data { before: pre_dir_attr, after: post_dir_attr };

            debug!("nfsproc3_link success");
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            nfs3::nfsstat3::NFS3_OK.serialize(output)?;
            file_attr.serialize(output)?;
            wcc_res.serialize(output)?;
        }
        Err(stat) => {
            // Get file attributes
            let file_attr = context.vfs.getattr(fileid).await.ok();

            // Get the directory attributes after the operation (unchanged)
            let post_dir_attr = context.vfs.getattr(dirid).await.ok();

            let wcc_res = nfs3::wcc_data { before: pre_dir_attr, after: post_dir_attr };

            debug!("nfsproc3_link failed: {:?}", stat);
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            stat.serialize(output)?;
            file_attr.serialize(output)?;
            wcc_res.serialize(output)?;
        }
    }

    Ok(())
}
