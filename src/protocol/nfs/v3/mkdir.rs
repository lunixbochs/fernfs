//! Implementation of the MKDIR procedure (procedure 9) for NFS version 3 protocol
//! as defined in RFC 1813 section 3.3.9.
//!
//! The MKDIR procedure creates a new directory in the specified parent directory.
//! The client specifies:
//! - The file handle of the parent directory
//! - The name of the new directory
//! - The initial attributes for the new directory
//!
//! On successful return, the server provides:
//! - The file handle of the new directory
//! - The attributes of the new directory
//! - The attributes of the parent directory before and after the operation (weak cache consistency)
//!
//! This procedure fails if the parent directory is read-only, the name already exists,
//! or the user doesn't have appropriate access permissions.

use std::io::{Read, Write};

use tracing::{debug, error, warn};

use crate::protocol::rpc;
use crate::protocol::xdr::{self, deserialize, nfs3, Serialize};
use crate::vfs;

/// Handles NFSv3 MKDIR procedure (procedure 9)
///
/// MKDIR creates a new directory.
/// Takes parent directory handle, name for new directory and attributes.
/// Returns file handle and attributes of the newly created directory.
///
/// # Arguments
///
/// * `xid` - RPC transaction ID
/// * `input` - Input stream containing the MKDIR arguments
/// * `output` - Output stream for writing the response
/// * `context` - Server context containing VFS
///
/// # Returns
///
/// * `Result<(), anyhow::Error>` - Ok(()) on success or an error
pub async fn nfsproc3_mkdir(
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
    let args = deserialize::<nfs3::dir::MKDIR3args>(input)?;

    debug!("nfsproc3_mkdir({:?}, {:?}) ", xid, args);

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

    let res = context.vfs.mkdir(dirid, &args.dirops.name).await;

    // Re-read dir attributes for post op attr
    let post_dir_attr = context.vfs.getattr(dirid).await.ok();
    let wcc_res = nfs3::wcc_data { before: pre_dir_attr, after: post_dir_attr };

    match res {
        Ok((fid, fattr)) => {
            debug!("mkdir success --> {:?}, {:?}", fid, fattr);
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            nfs3::nfsstat3::NFS3_OK.serialize(output)?;
            // serialize CREATE3resok
            let fh = context.vfs.id_to_fh(fid);
            nfs3::post_op_fh3::Some(fh).serialize(output)?;
            nfs3::post_op_attr::Some(fattr).serialize(output)?;
            wcc_res.serialize(output)?;
        }
        Err(e) => {
            debug!("mkdir error {:?} --> {:?}", xid, e);
            // serialize CREATE3resfail
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            e.serialize(output)?;
            wcc_res.serialize(output)?;
        }
    }

    Ok(())
}
