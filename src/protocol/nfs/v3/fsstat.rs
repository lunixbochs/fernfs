//! Implementation of the FSSTAT procedure (procedure 18) for NFS version 3 protocol
//! as defined in RFC 1813 section 3.3.18.
//!
//! The FSSTAT procedure retrieves volatile file system state information.
//! This differs from the FSINFO procedure which returns static information about
//! the file system. FSSTAT provides statistics about total and free space, total
//! and free file slots, and other dynamic information.
//!
//! The client specifies:
//! - A file handle representing the file system (typically the root file handle)
//!
//! On successful return, the server provides:
//! - The attributes of the file handle provided
//! - Total bytes in the file system
//! - Free bytes in the file system
//! - Available bytes to the user (accounting for quotas)
//! - Total file slots in the file system
//! - Free file slots in the file system
//! - Available file slots to the user (accounting for quotas)
//! - How long this information remains valid (invarsec)

use std::io::{Read, Write};

use tracing::debug;

use crate::protocol::rpc;
use crate::protocol::xdr::{self, deserialize, nfs3, Serialize};

/// Handles NFSv3 FSSTAT procedure (procedure 18)
///
/// FSSTAT retrieves volatile file system state information.
/// Takes a file handle representing the file system.
/// Returns file attributes and dynamic file system information.
///
/// # Arguments
///
/// * `xid` - RPC transaction ID
/// * `input` - Input stream containing the FSSTAT arguments
/// * `output` - Output stream for writing the response
/// * `context` - Server context containing VFS
///
/// # Returns
///
/// * `Result<(), anyhow::Error>` - Ok(()) on success or an error
pub async fn nfsproc3_fsstat(
    xid: u32,
    input: &mut impl Read,
    output: &mut impl Write,
    context: &rpc::Context,
) -> Result<(), anyhow::Error> {
    let handle = deserialize::<nfs3::nfs_fh3>(input)?;
    debug!("nfsproc3_fsstat({:?},{:?}) ", xid, handle);
    let id = context.vfs.fh_to_id(&handle);
    // fail if unable to convert file handle
    if let Err(stat) = id {
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        stat.serialize(output)?;
        nfs3::post_op_attr::None.serialize(output)?;
        return Ok(());
    }
    let id = id.unwrap();

    let obj_attr = context.vfs.getattr(id).await.ok();
    let res = nfs3::fs::FSSTAT3resok {
        obj_attributes: obj_attr,
        tbytes: 1024 * 1024 * 1024 * 1024,
        fbytes: 1024 * 1024 * 1024 * 1024,
        abytes: 1024 * 1024 * 1024 * 1024,
        tfiles: 1024 * 1024 * 1024,
        ffiles: 1024 * 1024 * 1024,
        afiles: 1024 * 1024 * 1024,
        invarsec: u32::MAX,
    };
    xdr::rpc::make_success_reply(xid).serialize(output)?;
    nfs3::nfsstat3::NFS3_OK.serialize(output)?;
    debug!(" {:?} ---> {:?}", xid, res);
    res.serialize(output)?;
    Ok(())
}
