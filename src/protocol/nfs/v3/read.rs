//! Implementation of the READ procedure (procedure 6) for NFS version 3 protocol
//! as defined in RFC 1813 section 3.3.6.
//!
//! The READ procedure retrieves data from a regular file. It may be used to read
//! whole or partial files at any offset. The client specifies:
//! - The file handle of the file to read
//! - The offset in the file to start reading
//! - The amount of data to read
//!
//! On successful return, the server provides:
//! - The file attributes after the read
//! - The actual number of bytes read
//! - An EOF flag indicating whether the read reached the end of file
//! - The data read from the file

use std::io::{Read, Write};

use tracing::{debug, error};

use crate::protocol::rpc;
use crate::protocol::xdr::{self, deserialize, nfs3, Serialize};

/// Handles NFSv3 READ procedure (procedure 6)
///
/// READ retrieves data from a file.
/// It takes file handle, offset and byte count to read.
/// Returns file attributes, read data and EOF indicator.
///
/// # Arguments
///
/// * `xid` - RPC transaction ID
/// * `input` - Input stream containing the READ arguments
/// * `output` - Output stream for writing the response
/// * `context` - Server context containing VFS
///
/// # Returns
///
/// * `Result<(), anyhow::Error>` - Ok(()) on success or an error
pub async fn nfsproc3_read(
    xid: u32,
    input: &mut impl Read,
    output: &mut impl Write,
    context: &rpc::Context,
) -> Result<(), anyhow::Error> {
    let args = deserialize::<nfs3::file::READ3args>(input)?;
    debug!("nfsproc3_read({:?},{:?}) ", xid, args);

    let id = context.vfs.fh_to_id(&args.file);
    if let Err(stat) = id {
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        stat.serialize(output)?;
        nfs3::post_op_attr::None.serialize(output)?;
        return Ok(());
    }
    let id = id.unwrap();

    let obj_attr = context.vfs.getattr(id).await.ok();
    match context.vfs.read(id, args.offset, args.count).await {
        Ok((bytes, eof)) => {
            let res = nfs3::file::READ3resok {
                file_attributes: obj_attr,
                count: bytes.len() as u32,
                eof,
                data: bytes,
            };
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            nfs3::nfsstat3::NFS3_OK.serialize(output)?;
            res.serialize(output)?;
        }
        Err(stat) => {
            error!("nfsproc3_read error {:?} --> {:?}", xid, stat);
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            stat.serialize(output)?;
            obj_attr.serialize(output)?;
        }
    }
    Ok(())
}
