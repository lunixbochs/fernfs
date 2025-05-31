//! Implementation of the EXPORT procedure (procedure 5) for MOUNT version 3 protocol
//! as defined in RFC 1813 Appendix I section I.4.5.
//!
//! The EXPORT procedure provides a list of all exported file systems and their
//! associated access control lists. This is typically used by clients to discover
//! available mount points on the server.
//!
//! EXPORT takes no arguments and returns a list of exported file systems.
//! Each export entry includes a directory path and optional access groups list.

use std::io::Write;

use tracing::debug;

use crate::protocol::rpc;
use crate::protocol::xdr::{self, XDR};

/// Handles MOUNT protocol EXPORT procedure (procedure 5)
///
/// EXPORT retrieves a list of exported file systems.
/// Takes no arguments and returns the list of all available exports.
/// In this implementation, returns only the configured export path.
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
    // dirpath
    context.export_name.as_bytes().to_vec().serialize(output)?;
    // groups
    false.serialize(output)?;
    // next exports
    false.serialize(output)?;
    Ok(())
}
