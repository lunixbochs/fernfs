//! Implementation of the WRITE procedure (procedure 7) for NFS version 3 protocol
//! as defined in RFC 1813 section 3.3.7.
//!
//! The WRITE procedure writes data to a regular file. It can be used for
//! creating a file (with the CREATE procedure) or appending data to a file.
//! The client specifies:
//! - The file handle of the file to which data is to be written
//! - The offset within the file where the write should begin
//! - The amount of data to be written (count)
//! - A stability level (UNSTABLE, DATA_SYNC, or FILE_SYNC)
//! - The data to be written
//!
//! On successful return, the server provides:
//! - The file attributes before and after the write (weak cache consistency)
//! - The number of bytes actually written
//! - The stability level used for the write
//! - A write verifier to detect server restarts

use std::io::{Read, Write};

use tracing::{debug, error, warn};

use crate::protocol::rpc;
use crate::protocol::xdr::{self, deserialize, nfs3, Serialize};
use crate::vfs;

/// Handles NFSv3 WRITE procedure (procedure 7)
///
/// WRITE writes data to a file on the server.
/// It takes file handle, offset, stability flag and data to write.
/// Returns amount of data written and file attributes after the operation.
///
/// # Arguments
///
/// * `xid` - RPC transaction ID
/// * `input` - Input stream containing the WRITE arguments
/// * `output` - Output stream for writing the response
/// * `context` - Server context containing VFS
///
/// # Returns
///
/// * `Result<(), anyhow::Error>` - Ok(()) on success or an error
pub async fn nfsproc3_write(
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

    let args = deserialize::<nfs3::file::WRITE3args>(input)?;
    debug!("nfsproc3_write({:?},...) ", xid);
    // sanity check the length
    if args.data.len() != args.count as usize {
        xdr::rpc::garbage_args_reply_message(xid).serialize(output)?;
        return Ok(());
    }

    let id = context.vfs.fh_to_id(&args.file);
    if let Err(stat) = id {
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        stat.serialize(output)?;
        nfs3::wcc_data::default().serialize(output)?;
        return Ok(());
    }
    let id = id.unwrap();

    // get the object attributes before the write
    let pre_obj_attr = context
        .vfs
        .getattr(id)
        .await
        .map(|v| nfs3::wcc_attr { size: v.size, mtime: v.mtime, ctime: v.ctime })
        .ok();

    match context.vfs.write(id, args.offset, &args.data).await {
        Ok(fattr) => {
            debug!("write success {:?} --> {:?}", xid, fattr);
            let res = nfs3::file::WRITE3resok {
                file_wcc: nfs3::wcc_data {
                    before: pre_obj_attr,
                    after: nfs3::post_op_attr::Some(fattr),
                },
                count: args.count,
                committed: nfs3::file::stable_how::FILE_SYNC,
                verf: context.vfs.server_id(),
            };
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            nfs3::nfsstat3::NFS3_OK.serialize(output)?;
            res.serialize(output)?;
        }
        Err(stat) => {
            error!("write error {:?} --> {:?}", xid, stat);
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            stat.serialize(output)?;
            nfs3::wcc_data::default().serialize(output)?;
        }
    }
    Ok(())
}
