//! MOUNT protocol implementation for NFS version 3 as specified in RFC 1813 section 5.0.
//! https://datatracker.ietf.org/doc/html/rfc1813#section-5.0

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

/// Main handler for MOUNT procedures of version 3 protocol.
///
/// TODO: MOUNTPROC3_DUMP function is not implemented.
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
