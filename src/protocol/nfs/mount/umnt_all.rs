//! Implementation of the UMNTALL procedure (procedure 4) for MOUNT version 3 protocol
//! as defined in RFC 1813 Appendix I section I.4.4.
//!
//! The UMNTALL procedure removes all mount entries for the client from the server's mount list.
//! This is usually called when a client is shutting down or when all mounted file systems
//! need to be unmounted at once.
//!
//! UMNTALL takes no arguments and returns nothing.

use std::io::Write;

use tracing::debug;

use crate::protocol::rpc;
use crate::protocol::xdr::{self, mount, XDR};

/// Handles MOUNT protocol UMNTALL procedure (procedure 4)
///
/// UMNTALL removes all mounts made by the client.
/// Takes no arguments and unmounts all client mount points.
/// Sends unmount notification signal if configured.
///
/// # Arguments
///
/// * `xid` - RPC transaction ID
/// * `output` - Output stream for writing the response
/// * `context` - Server context containing mount signal information
///
/// # Returns
///
/// * `Result<(), anyhow::Error>` - Ok(()) on success or an error
pub async fn mountproc3_umnt_all(
    xid: u32,
    output: &mut impl Write,
    context: &rpc::Context,
) -> Result<(), anyhow::Error> {
    debug!("mountproc3_umnt_all({:?}) ", xid);
    if let Some(ref chan) = context.mount_signal {
        let _ = chan.send(false).await;
    }
    xdr::rpc::make_success_reply(xid).serialize(output)?;
    mount::mountstat3::MNT3_OK.serialize(output)?;
    Ok(())
}
