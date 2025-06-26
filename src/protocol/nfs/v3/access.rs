//! Implementation of the ACCESS procedure (procedure 4) for NFS version 3 protocol
//! as defined in RFC 1813 section 3.3.4.
//!
//! The ACCESS procedure determines the access rights that a user, as identified
//! by the authentication credentials, has with respect to a file system object.
//! The client uses the returned access information to:
//!
//! - Cache access permissions to avoid unnecessary RPC calls
//! - Validate user access before executing operations
//! - Determine actual access rights when file mode bits are inadequate
//!
//! This procedure improves over NFSv2 by providing explicit access right checking
//! rather than requiring clients to attempt operations to discover allowed actions.

use std::io::{Read, Write};

use tracing::debug;

use crate::protocol::rpc;
use crate::protocol::xdr::{self, deserialize, nfs3, Serialize};
use crate::vfs;

/// Handles NFSv3 ACCESS procedure (procedure 4)
///
/// ACCESS determines the access rights a user has to a file system object.
/// It evaluates requested access permissions against the file's attributes
/// and returns which operations client is allowed to perform.
///
/// The client encodes the following types of requested permissions in the
/// 'access' parameter:
/// - ACCESS3_READ (0x0001): Read data from file or read a directory
/// - ACCESS3_LOOKUP (0x0002): Look up a name in a directory
/// - ACCESS3_MODIFY (0x0004): Modify a file's data
/// - ACCESS3_EXTEND (0x0008): Extend a file or directory
/// - ACCESS3_DELETE (0x0010): Delete a file or directory
/// - ACCESS3_EXECUTE (0x0020): Execute a file or traverse a directory
///
/// # Arguments
///
/// * `xid` - RPC transaction ID
/// * `input` - Input stream containing the file handle and access mask
/// * `output` - Output stream for writing the response
/// * `context` - Server context containing VFS
///
/// # Returns
///
/// * `Result<(), anyhow::Error>` - Ok(()) on success or an error
pub async fn nfsproc3_access(
    xid: u32,
    input: &mut impl Read,
    output: &mut impl Write,
    context: &rpc::Context,
) -> Result<(), anyhow::Error> {
    let handle = deserialize::<nfs3::nfs_fh3>(input)?;
    let access = deserialize::<u32>(input)?;
    debug!("nfsproc3_access({:?},{:?},{:?})", xid, handle, access);

    let id = context.vfs.fh_to_id(&handle);
    // Fail if unable to convert file handle
    if let Err(stat) = id {
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        stat.serialize(output)?;
        nfs3::post_op_attr::None.serialize(output)?;
        return Ok(());
    }
    let id = id.unwrap();

    // Get object attributes
    let obj_attr = match context.vfs.getattr(id).await {
        Ok(v) => nfs3::post_op_attr::Some(v),
        Err(stat) => {
            // If we can't get attributes, return an error
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            stat.serialize(output)?;
            nfs3::post_op_attr::None.serialize(output)?;
            return Ok(());
        }
    };

    // Check if the object exists
    if obj_attr.is_none() {
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        nfs3::nfsstat3::NFS3ERR_NOENT.serialize(output)?;
        nfs3::post_op_attr::None.serialize(output)?;
        return Ok(());
    }

    // Extract object attributes
    let attr = match obj_attr {
        nfs3::post_op_attr::Some(attr) => attr,
        None => {
            // This should not happen, since we already checked that obj_attr is not Void
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            nfs3::nfsstat3::NFS3ERR_SERVERFAULT.serialize(output)?;
            nfs3::post_op_attr::None.serialize(output)?;
            return Ok(());
        }
    };

    // Check access permissions based on file type and attributes
    let mut granted_access = 0;

    // Always allow LOOKUP for existing objects
    granted_access |= nfs3::ACCESS3_LOOKUP;

    // Check permissions based on file type
    match attr.ftype {
        nfs3::ftype3::NF3REG => {
            // For regular files
            if !matches!(context.vfs.capabilities(), vfs::Capabilities::ReadWrite) {
                // If the file system is read-only, allow only reading
                if access & (nfs3::ACCESS3_READ | nfs3::ACCESS3_EXECUTE) != 0 {
                    granted_access |= access & (nfs3::ACCESS3_READ | nfs3::ACCESS3_EXECUTE);
                }
            } else {
                // If the file system supports read and write, check access permissions
                // Here you can add a check for real access permissions based on file attributes
                // For example, check owner, group, and access permissions

                // For simplicity, allow all requested permissions
                granted_access |= access;
            }
        }
        nfs3::ftype3::NF3DIR => {
            // For directories
            if !matches!(context.vfs.capabilities(), vfs::Capabilities::ReadWrite) {
                // If the file system is read-only, allow only reading
                if access & nfs3::ACCESS3_READ != 0 {
                    granted_access |= nfs3::ACCESS3_READ;
                }
            } else {
                // If the file system supports read and write, check access permissions
                // For directories, allow reading and execution
                if access & (nfs3::ACCESS3_READ | nfs3::ACCESS3_EXECUTE) != 0 {
                    granted_access |= access & (nfs3::ACCESS3_READ | nfs3::ACCESS3_EXECUTE);
                }

                // For operations that modify the directory, check write permissions
                if access & (nfs3::ACCESS3_MODIFY | nfs3::ACCESS3_EXTEND | nfs3::ACCESS3_DELETE)
                    != 0
                {
                    // Here you can add a check for real access permissions
                    granted_access |= access
                        & (nfs3::ACCESS3_MODIFY | nfs3::ACCESS3_EXTEND | nfs3::ACCESS3_DELETE);
                }
            }
        }
        nfs3::ftype3::NF3LNK => {
            // For symbolic links, allow only reading
            if access & nfs3::ACCESS3_READ != 0 {
                granted_access |= nfs3::ACCESS3_READ;
            }
        }
        _ => {
            // For other file types (devices, sockets, etc.)
            // Allow only reading and execution
            if access & (nfs3::ACCESS3_READ | nfs3::ACCESS3_EXECUTE) != 0 {
                granted_access |= access & (nfs3::ACCESS3_READ | nfs3::ACCESS3_EXECUTE);
            }
        }
    }

    debug!(" {:?} ---> {:?}", xid, granted_access);
    xdr::rpc::make_success_reply(xid).serialize(output)?;
    nfs3::nfsstat3::NFS3_OK.serialize(output)?;
    obj_attr.serialize(output)?;
    granted_access.serialize(output)?;
    Ok(())
}
