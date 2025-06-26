//! Implementation of the READLINK procedure (procedure 5) for NFS version 3 protocol
//! as defined in RFC 1813 section 3.3.5.
//!
//! The READLINK procedure reads the data associated with a symbolic link.
//! The server will read the link data to determine the file system object that the
//! symbolic link references.
//!
//! The client specifies:
//! - The file handle for the symbolic link
//!
//! On successful return, the server provides:
//! - The attributes of the symbolic link
//! - The path string contained in the symbolic link
//!
//! If the file handle passed to this procedure does not refer to a symbolic link,
//! the server should return NFS3ERR_INVAL. The READLINK operation is only allowed on
//! objects of type NF3LNK.

use std::io::{Read, Write};

use tracing::debug;

use crate::protocol::rpc;
use crate::protocol::xdr::{self, deserialize, nfs3, Serialize};

/// Handles NFSv3 READLINK procedure (procedure 5)
///
/// READLINK reads the data associated with a symbolic link.
/// Takes a file handle representing the symbolic link.
/// Returns the string contents of the symbolic link and file attributes.
///
/// # Arguments
///
/// * `xid` - RPC transaction ID
/// * `input` - Input stream containing the READLINK arguments
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
/// - NFS3ERR_INVAL - If the object is not a symbolic link
/// - NFS3ERR_IO - If the symbolic link cannot be read by the server
/// - NFS3ERR_STALE - If the file handle is invalid
pub async fn nfsproc3_readlink(
    xid: u32,
    input: &mut impl Read,
    output: &mut impl Write,
    context: &rpc::Context,
) -> Result<(), anyhow::Error> {
    let handle = deserialize::<nfs3::nfs_fh3>(input)?;
    debug!("nfsproc3_readlink({:?},{:?}) ", xid, handle);

    let id = context.vfs.fh_to_id(&handle);
    // fail if unable to convert file handle
    if let Err(stat) = id {
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        stat.serialize(output)?;
        return Ok(());
    }

    let id = id.unwrap();
    // if the id does not exist, we fail
    let symlink_attr = match context.vfs.getattr(id).await {
        Ok(v) => nfs3::post_op_attr::Some(v),
        Err(stat) => {
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            stat.serialize(output)?;
            nfs3::post_op_attr::None.serialize(output)?;
            return Ok(());
        }
    };

    match context.vfs.readlink(id).await {
        Ok(path) => {
            debug!(" {:?} --> {:?}", xid, path);
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            nfs3::nfsstat3::NFS3_OK.serialize(output)?;
            symlink_attr.serialize(output)?;
            path.serialize(output)?;
        }
        Err(stat) => {
            // failed to read link
            // retry with failure and the post_op_attr
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            stat.serialize(output)?;
            symlink_attr.serialize(output)?;
        }
    }

    Ok(())
}
