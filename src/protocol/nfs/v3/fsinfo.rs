//! Implementation of the FSINFO procedure (procedure 19) for NFS version 3 protocol
//! as defined in RFC 1813 section 3.3.19.
//!
//! The FSINFO procedure retrieves static information about the file system that
//! is exported by the server. It is used by NFS clients to determine various
//! configuration parameters and capabilities of the server implementation.
//!
//! The client specifies:
//! - A file handle (typically the root file handle of the exported file system)
//!
//! On successful return, the server provides:
//! - The file attributes for the file handle provided
//! - Maximum and preferred read and write transfer sizes
//! - Preferred directory read size
//! - Server time precision (time_delta)
//! - The file system properties (whether it supports hard links, symbolic links, etc.)
//! - The maximum file size supported by the server

use std::io::{Read, Write};

use tracing::{debug, error};

use crate::protocol::rpc;
use crate::protocol::xdr::{self, deserialize, nfs3, Serialize};

/// Handles NFSv3 FSINFO procedure (procedure 19)
///
/// FSINFO retrieves static file system information.
/// Takes a file handle representing the file system.
/// Returns various file system parameters and capabilities.
///
/// # Arguments
///
/// * `xid` - RPC transaction ID
/// * `input` - Input stream containing the FSINFO arguments
/// * `output` - Output stream for writing the response
/// * `context` - Server context containing VFS
///
/// # Returns
///
/// * `Result<(), anyhow::Error>` - Ok(()) on success or an error
pub async fn nfsproc3_fsinfo(
    xid: u32,
    input: &mut impl Read,
    output: &mut impl Write,
    context: &rpc::Context,
) -> Result<(), anyhow::Error> {
    let handle = deserialize::<nfs3::nfs_fh3>(input)?;
    debug!("nfsproc3_fsinfo({:?},{:?}) ", xid, handle);

    let id = context.vfs.fh_to_id(&handle);
    // fail if unable to convert file handle
    if let Err(stat) = id {
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        stat.serialize(output)?;
        nfs3::post_op_attr::None.serialize(output)?;
        return Ok(());
    }
    let id = id.unwrap();

    match context.vfs.fsinfo(id).await {
        Ok(fsinfo) => {
            debug!(" {:?} --> {:?}", xid, fsinfo);
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            nfs3::nfsstat3::NFS3_OK.serialize(output)?;
            fsinfo.serialize(output)?;
        }
        Err(stat) => {
            error!("nfsproc3_fsinfo error {:?} --> {:?}", xid, stat);
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            stat.serialize(output)?;
        }
    }
    Ok(())
}
