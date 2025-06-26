//! Implementation of the PATHCONF procedure (procedure 20) for NFS version 3 protocol
//! as defined in RFC 1813 section 3.3.20.
//!
//! The PATHCONF procedure retrieves the pathconf information for a file or
//! directory. This information is typically used by clients to determine
//! various file system characteristics to properly format and display file names
//! and paths.
//!
//! The client specifies:
//! - A file handle for a file or directory
//!
//! On successful return, the server provides:
//! - The file attributes for the file handle provided
//! - Maximum link count for a file (number of hard links)
//! - Maximum length for a file name
//! - Whether the file system enforces file name truncation or returns errors for long names
//! - Whether the file system restricts ownership changes
//! - Whether file names are case-insensitive
//! - Whether file names are case-preserving

use std::io::{Read, Write};

use tracing::debug;

use crate::protocol::rpc;
use crate::protocol::xdr::{self, deserialize, nfs3, Serialize};

/// Handles NFSv3 PATHCONF procedure (procedure 20)
///
/// PATHCONF retrieves file system path configuration information.
/// Takes a file handle representing the file system.
/// Returns parameters like maximum link count, maximum name length, etc.
///
/// # Arguments
///
/// * `xid` - RPC transaction ID
/// * `input` - Input stream containing the PATHCONF arguments
/// * `output` - Output stream for writing the response
/// * `context` - Server context containing VFS
///
/// # Returns
///
/// * `Result<(), anyhow::Error>` - Ok(()) on success or an error
pub async fn nfsproc3_pathconf(
    xid: u32,
    input: &mut impl Read,
    output: &mut impl Write,
    context: &rpc::Context,
) -> Result<(), anyhow::Error> {
    let handle = deserialize::<nfs3::nfs_fh3>(input)?;
    debug!("nfsproc3_pathconf({:?},{:?})", xid, handle);

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
    let res = nfs3::fs::PATHCONF3resok {
        obj_attributes: obj_attr,
        linkmax: 0,
        name_max: 32768,
        no_trunc: true,
        chown_restricted: true,
        case_insensitive: false,
        case_preserving: true,
    };
    debug!(" {:?} ---> {:?}", xid, res);
    xdr::rpc::make_success_reply(xid).serialize(output)?;
    nfs3::nfsstat3::NFS3_OK.serialize(output)?;
    res.serialize(output)?;
    Ok(())
}
