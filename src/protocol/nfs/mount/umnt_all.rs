//! Implementation of the UMNTALL procedure (procedure 4) for MOUNT version 3 protocol
//! as defined in RFC 1813 section 5.2.4
//! https://datatracker.ietf.org/doc/html/rfc1813#section-5.2.4

use std::io::Write;

use tracing::debug;

use crate::protocol::rpc;
use crate::protocol::xdr::{self, mount, XDR};

/// Handles MOUNTPROC3_UMNTALL procedure.
///
/// Function removes all of the mount entries for
/// this client previously recorded by calls to MNT.
///
/// TODO: Currently we have only one mount point,
/// if there will be more, we need to extend functionality.
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
