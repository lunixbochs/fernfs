//! Implementation of the SYMLINK procedure (procedure 10) for NFS version 3 protocol
//! as defined in RFC 1813 section 3.3.10.
//!
//! The SYMLINK procedure creates a symbolic link in a directory. A symbolic link
//! is a special type of file that contains a path name that clients can use to
//! reference another file or directory, possibly on a different file system or server.
//!
//! The client specifies:
//! - The file handle and filename for the directory and name of the link to be created
//! - The path string that the symbolic link will contain
//! - Attributes to set on the newly created symbolic link
//!
//! On successful return, the server provides:
//! - The file handle of the newly created symbolic link
//! - The attributes of the newly created symbolic link
//! - The attributes of the directory before and after the operation (weak cache consistency)
//!
//! Note that the path contained in a symbolic link is not validated by the server and
//! may point to a nonexistent file or a file on another server. The symbolic link
//! target path resolves when the client accesses the link through a path traversal
//! operation, typically via LOOKUP or READLINK procedures.
//!
//! Common errors include:
//! - NFS3ERR_ROFS - If the file system is read-only
//! - NFS3ERR_EXIST - If the target name already exists
//! - NFS3ERR_ACCES - If the client doesn't have permission to create the symlink
//! - NFS3ERR_NOSPC - If there is insufficient storage space

use std::io::{Read, Write};

use tracing::{debug, error, warn};

use crate::protocol::rpc;
use crate::protocol::xdr::{self, deserialize, nfs3, Serialize};
use crate::vfs;

/// Handles NFSv3 SYMLINK procedure (procedure 10)
///
/// SYMLINK creates a symbolic link to a specified target path.
/// Takes directory handle, name for new link, and target path data.
/// Returns file handle and attributes of the created symbolic link.
///
/// # Arguments
///
/// * `xid` - RPC transaction ID
/// * `input` - Input stream containing the SYMLINK arguments
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
/// - NFS3ERR_EXIST - If a file with the requested name already exists
/// - NFS3ERR_ACCES - If permission is denied
/// - NFS3ERR_NOSPC - If there is no space on the file system
/// - NFS3ERR_NOTDIR - If the parent handle is not a directory
/// - NFS3ERR_STALE - If the file handle is invalid
pub async fn nfsproc3_symlink(
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
    let args = deserialize::<nfs3::dir::SYMLINK3args>(input)?;

    debug!("nfsproc3_symlink({:?}, {:?}) ", xid, args);

    // find the directory we are supposed to create the
    // new file in
    let dirid = context.vfs.fh_to_id(&args.dirops.dir);
    if let Err(stat) = dirid {
        // directory does not exist
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        stat.serialize(output)?;
        nfs3::wcc_data::default().serialize(output)?;
        error!("Directory does not exist");
        return Ok(());
    }
    // found the directory, get the attributes
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

    let res = context
        .vfs
        .symlink(
            dirid,
            &args.dirops.name,
            &args.symlink.symlink_data,
            &args.symlink.symlink_attributes,
        )
        .await;

    // Re-read dir attributes for post op attr
    let post_dir_attr = context.vfs.getattr(dirid).await.ok();
    let wcc_res = nfs3::wcc_data { before: pre_dir_attr, after: post_dir_attr };

    match res {
        Ok((fid, fattr)) => {
            debug!("symlink success --> {:?}, {:?}", fid, fattr);
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            nfs3::nfsstat3::NFS3_OK.serialize(output)?;
            // serialize CREATE3resok
            let fh = context.vfs.id_to_fh(fid);
            nfs3::post_op_fh3::Some(fh).serialize(output)?;
            nfs3::post_op_attr::Some(fattr).serialize(output)?;
            wcc_res.serialize(output)?;
        }
        Err(e) => {
            debug!("symlink error --> {:?}", e);
            // serialize CREATE3resfail
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            e.serialize(output)?;
            wcc_res.serialize(output)?;
        }
    }

    Ok(())
}
