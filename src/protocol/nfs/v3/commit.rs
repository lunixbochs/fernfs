//! Implementation of the COMMIT procedure (procedure 21) for NFS version 3 protocol
//! as defined in RFC 1813 section 3.3.21.
//!
//! The COMMIT procedure forces or flushes data to stable storage that was previously
//! written with a WRITE procedure call with the stable flag set to UNSTABLE.
//!
//! The client specifies:
//! - The file handle of the file to which data is to be flushed
//! - The offset within the file where flushing should begin
//! - The count of bytes to flush (0 means flush all data from the offset to the end of file)
//!
//! This procedure implements a two-phase commit strategy:
//! 1. Client sends WRITE requests with unstable flag to improve performance
//! 2. Client later sends COMMIT to ensure data durability
//!
//! On successful return, the server provides:
//! - The file attributes before and after the operation
//! - A write verifier that the client can compare with the one from previous WRITEs
//!   to detect server reboots that might have lost uncommitted data

use std::io::{Read, Write};

use tracing::debug;

use crate::protocol::rpc;
use crate::protocol::xdr::{self, deserialize, nfs3, Serialize};

/// Handles NFSv3 COMMIT procedure (procedure 21)
///
/// COMMIT forces or flushes cached data to stable storage.
/// It takes a file handle, starting offset and byte count to commit.
/// Returns file attributes and write verifier after the operation.
///
/// # Arguments
///
/// * `xid` - RPC transaction ID
/// * `input` - Input stream containing the COMMIT arguments
/// * `output` - Output stream for writing the response
/// * `context` - Server context containing VFS
///
/// # Returns
///
/// * `Result<(), anyhow::Error>` - Ok(()) on success or an error
pub async fn nfsproc3_commit(
    xid: u32,
    input: &mut impl Read,
    output: &mut impl Write,
    context: &rpc::Context,
) -> Result<(), anyhow::Error> {
    let args = deserialize::<nfs3::file::COMMIT3args>(input)?;
    debug!("nfsproc3_commit({:?}, {:?}) ", xid, args);

    let id = context.vfs.fh_to_id(&args.file);
    // fail if unable to convert file handle
    if let Err(stat) = id {
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        stat.serialize(output)?;
        nfs3::wcc_data::default().serialize(output)?;
        return Ok(());
    }
    let id = id.unwrap();

    // get the object attributes before the commit
    let pre_obj_attr = context
        .vfs
        .getattr(id)
        .await
        .map(|v| nfs3::wcc_attr { size: v.size, mtime: v.mtime, ctime: v.ctime })
        .ok();

    // Call VFS commit method
    match context.vfs.commit(id, args.offset, args.count).await {
        Ok(fattr) => {
            let post_obj_attr = nfs3::post_op_attr::Some(fattr);

            let res = nfs3::file::COMMIT3resok {
                file_wcc: nfs3::wcc_data { before: pre_obj_attr, after: post_obj_attr },
                verf: context.vfs.server_id(),
            };

            debug!("nfsproc3_commit success");
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            nfs3::nfsstat3::NFS3_OK.serialize(output)?;
            res.serialize(output)?;
        }
        Err(stat) => {
            let post_obj_attr = context.vfs.getattr(id).await.ok();

            let wcc_data = nfs3::wcc_data { before: pre_obj_attr, after: post_obj_attr };

            debug!("nfsproc3_commit error: {:?}", stat);
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            stat.serialize(output)?;
            wcc_data.serialize(output)?;
        }
    }

    Ok(())
}
