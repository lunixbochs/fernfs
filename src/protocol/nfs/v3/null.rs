//! Implementation of the NULL procedure (procedure 0) for NFS version 3 protocol
//! as defined in RFC 1813 section 3.3.0.
//!
//! The NULL procedure does no work and is typically used to:
//! - Check if the server is responding (ping)
//! - Measure basic RPC round-trip time
//! - Validate RPC credentials
//!
//! NULL takes no arguments and returns no results, just an RPC response indicating success.

use std::io::Write;

use tracing::debug;

use crate::protocol::xdr::{self, XDR};

/// Handles NFSv3 NULL procedure
///
/// NULL is a no-operation RPC call used to check if the server is responding.
/// Takes no arguments and returns nothing but an RPC success.
pub fn nfsproc3_null(xid: u32, output: &mut impl Write) -> Result<(), anyhow::Error> {
    debug!("nfsproc3_null({:?}) ", xid);
    let msg = xdr::rpc::make_success_reply(xid);
    debug!("\t{:?} --> {:?}", xid, msg);
    msg.serialize(output)?;
    Ok(())
}
