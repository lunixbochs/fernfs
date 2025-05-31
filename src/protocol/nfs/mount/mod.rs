//! MOUNT protocol implementation for NFS version 3 as specified in RFC 1813 Appendix I.
//!
//! The MOUNT protocol is used by NFS clients to:
//! - Obtain the initial file handle for a file system exported by the server
//! - Get a list of available exports from the server
//! - Notify the server when a mounted file system is no longer in use
//!
//! This module implements all 6 procedures defined in the MOUNT version 3 protocol:
//!
//! 1. NULL - Do nothing (ping server)
//! 2. MNT - Mount a file system and get its root file handle
//! 3. DUMP - List all mounted file systems (not implemented)
//! 4. UMNT - Unmount a file system
//! 5. UMNTALL - Unmount all file systems
//! 6. EXPORT - List available exports
//!
//! The MOUNT protocol serves several important purposes in the NFS ecosystem:
//! - Provides initial file system access through authenticated file handles
//! - Enables servers to track which clients have mounted their file systems
//! - Allows for graceful unmounting notification to release server resources
//! - Supports discovery of all available file systems through the EXPORT procedure
//!
//! RFC 1813 Appendix I defines the MOUNT protocol as an essential companion
//! to the main NFS protocol. It is typically used during the client's initialization
//! sequence before any NFS operations can occur.

use std::io::{Read, Write};

use num_traits::cast::FromPrimitive;

use crate::protocol::rpc;
use crate::protocol::xdr::{self, mount, XDR};

mod export;
mod mnt;
mod null;
mod umnt;
mod umnt_all;

use export::mountproc3_export;
use mnt::mountproc3_mnt;
use null::mountproc3_null;
use umnt::mountproc3_umnt;
use umnt_all::mountproc3_umnt_all;

/// Main handler for MOUNT protocol
///
/// Dispatches MOUNT protocol RPC calls to appropriate procedure handlers.
/// Provides operations for mounting, unmounting and export listing.
/// Used by clients to obtain initial file handles for file system access.
///
/// # Arguments
///
/// * `xid` - RPC transaction ID from the client
/// * `call` - The RPC call body containing program, version, and procedure numbers
/// * `input` - Input stream for reading procedure arguments
/// * `output` - Output stream for writing procedure results
/// * `context` - Server context containing exports and VFS information
///
/// # Returns
///
/// * `Result<(), anyhow::Error>` - Ok(()) on success or an error
pub async fn handle_mount(
    xid: u32,
    call: xdr::rpc::call_body,
    input: &mut impl Read,
    output: &mut impl Write,
    context: &rpc::Context,
) -> Result<(), anyhow::Error> {
    let prog = mount::MountProgram::from_u32(call.proc).unwrap_or(mount::MountProgram::INVALID);

    match prog {
        mount::MountProgram::MOUNTPROC3_NULL => mountproc3_null(xid, output)?,
        mount::MountProgram::MOUNTPROC3_MNT => mountproc3_mnt(xid, input, output, context).await?,
        mount::MountProgram::MOUNTPROC3_UMNT => {
            mountproc3_umnt(xid, input, output, context).await?
        }
        mount::MountProgram::MOUNTPROC3_UMNTALL => {
            mountproc3_umnt_all(xid, output, context).await?
        }
        mount::MountProgram::MOUNTPROC3_EXPORT => mountproc3_export(xid, output, context)?,
        _ => xdr::rpc::proc_unavail_reply_message(xid).serialize(output)?,
    }
    Ok(())
}
