//! Implementation of the SETATTR procedure (procedure 2) for NFS version 3 protocol
//! as defined in RFC 1813 section 3.3.2.
//!
//! The SETATTR procedure changes file system object attributes on the server.
//! It allows clients to modify mode, user/group ownership, size, access/modify times,
//! and other attributes of a file.
//!
//! The client specifies:
//! - The file handle for the file or directory to modify
//! - The attributes to set (sattr3 structure)
//! - An optional guard condition to prevent attribute modification race conditions
//!
//! The guard condition can be:
//! - NULL (no guard - always modify the attributes)
//! - A specific file ctime value - only modify if the current ctime matches
//!
//! On successful return, the server provides:
//! - The attributes of the object before and after the SETATTR operation (weak cache consistency)
//!
//! This procedure is critical for maintaining file consistency across multiple
//! clients that share access to the same files.

use std::io::{Read, Write};

use tracing::{debug, error, warn};

use crate::protocol::rpc;
use crate::protocol::xdr::{self, deserialize, nfs3, Serialize};
use crate::vfs;

/// Handles NFSv3 SETATTR procedure (procedure 2)
///
/// SETATTR changes the attributes of a file system object.
/// It takes a file handle, new attributes and optional guard condition.
/// Returns file attributes before and after the operation.
///
/// # Arguments
///
/// * `xid` - RPC transaction ID
/// * `input` - Input stream containing the SETATTR arguments
/// * `output` - Output stream for writing the response
/// * `context` - Server context containing VFS
///
/// # Returns
///
/// * `Result<(), anyhow::Error>` - Ok(()) on success or an error
pub async fn nfsproc3_setattr(
    xid: u32,
    input: &mut impl Read,
    output: &mut impl Write,
    context: &rpc::Context,
) -> Result<(), anyhow::Error> {
    if !matches!(context.vfs.capabilities(), vfs::Capabilities::ReadWrite) {
        warn!("No write capabilities.");
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        nfs3::nfsstat3::NFS3ERR_ROFS.serialize(output)?;
        nfs3::wcc_data::default().serialize(output)?;
        return Ok(());
    }
    let args = deserialize::<nfs3::SETATTR3args>(input)?;
    debug!("nfsproc3_setattr({:?},{:?}) ", xid, args);

    let id = context.vfs.fh_to_id(&args.object);
    // fail if unable to convert file handle
    if let Err(stat) = id {
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        stat.serialize(output)?;
        return Ok(());
    }
    let id = id.unwrap();

    let ctime;

    let pre_op_attr = match context.vfs.getattr(id).await {
        Ok(v) => {
            let wccattr = nfs3::wcc_attr { size: v.size, mtime: v.mtime, ctime: v.ctime };
            ctime = v.ctime;
            nfs3::pre_op_attr::Some(wccattr)
        }
        Err(stat) => {
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            stat.serialize(output)?;
            nfs3::wcc_data::default().serialize(output)?;
            return Ok(());
        }
    };
    // handle the guard
    if let nfs3::sattrguard3::Some(c) = args.guard {
        if c.seconds != ctime.seconds || c.nseconds != ctime.nseconds {
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            nfs3::nfsstat3::NFS3ERR_NOT_SYNC.serialize(output)?;
            nfs3::wcc_data::default().serialize(output)?;
        }
    }

    match context.vfs.setattr(id, args.new_attribute).await {
        Ok(post_op_attr) => {
            debug!(" setattr success {:?} --> {:?}", xid, post_op_attr);
            let wcc_res = nfs3::wcc_data {
                before: pre_op_attr,
                after: nfs3::post_op_attr::Some(post_op_attr),
            };
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            nfs3::nfsstat3::NFS3_OK.serialize(output)?;
            wcc_res.serialize(output)?;
        }
        Err(stat) => {
            error!("setattr error {:?} --> {:?}", xid, stat);
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            stat.serialize(output)?;
            nfs3::wcc_data::default().serialize(output)?;
        }
    }
    Ok(())
}
