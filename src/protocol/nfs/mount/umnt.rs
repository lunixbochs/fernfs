//! Implementation of the UMNT procedure (procedure 3) for MOUNT version 3 protocol
//! as defined in RFC 1813 section 5.2.3
//! https://datatracker.ietf.org/doc/html/rfc1813#section-5.2.3

use std::io::{Read, Write};

use tracing::debug;

use crate::protocol::rpc;
use crate::protocol::xdr::{self, mount, XDR};

/// Handles MOUNTPROC3_UMNT procedure.
///
/// Function removes the mount entry from the mount list for
/// the requested diretory.
///
/// TODO: Currently directory path is not used and only single
/// mount point is removed. Need to extend functionality.
///
/// # Arguments
///
/// * `xid` - RPC transaction ID
/// * `input` - Input stream containing the directory path to unmount
/// * `output` - Output stream for writing the response
/// * `context` - Server context containing mount signal information
///
/// # Returns
///
/// * `Result<(), anyhow::Error>` - Ok(()) on success or an error
pub async fn mountproc3_umnt(
    xid: u32,
    input: &mut impl Read,
    output: &mut impl Write,
    context: &rpc::Context,
) -> Result<(), anyhow::Error> {
    let mut path = mount::dirpath::new();
    path.deserialize(input)?;
    let utf8path = std::str::from_utf8(&path).unwrap_or_default();
    debug!("mountproc3_umnt({:?},{:?}) ", xid, utf8path);
    if let Some(ref chan) = context.mount_signal {
        let _ = chan.send(false).await;
    }
    xdr::rpc::make_success_reply(xid).serialize(output)?;
    mount::mountstat3::MNT3_OK.serialize(output)?;
    Ok(())
}
