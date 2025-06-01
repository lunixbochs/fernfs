//! Implementation of the EXPORT procedure (procedure 5) for MOUNT version 3 protocol
//! as defined in RFC 1813 section 5.2.5.
//! https://datatracker.ietf.org/doc/html/rfc1813#section-5.2.5

use std::io::Write;

use tracing::debug;

use crate::protocol::rpc;
use crate::protocol::xdr::{self, XDR};

/// Handles MOUNTPROC3_EXPORT procedure.
///
/// Function returns a list of all the exported file
/// systems and which clients are allowed to mount each one.
///
/// TODO: Currently function returns only one mount point in the list without groups.
///
/// # Arguments
///
/// * `xid` - RPC transaction ID
/// * `output` - Output stream for writing the response
/// * `context` - Server context containing export information
///
/// # Returns
///
/// * `Result<(), anyhow::Error>` - Ok(()) on success or an error
pub fn mountproc3_export(
    xid: u32,
    output: &mut impl Write,
    context: &rpc::Context,
) -> Result<(), anyhow::Error> {
    debug!("mountproc3_export({:?}) ", xid);
    xdr::rpc::make_success_reply(xid).serialize(output)?;
    true.serialize(output)?;
    // Dirpath of one export
    context.export_name.as_bytes().to_vec().serialize(output)?;
    // No groups
    false.serialize(output)?;
    // No next exports
    false.serialize(output)?;
    Ok(())
}
