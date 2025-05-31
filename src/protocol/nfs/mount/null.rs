//! Implementation of the NULL procedure (procedure 0) for MOUNT version 3 protocol
//! as defined in RFC 1813 Appendix I section I.4.1.
//!
//! The NULL procedure does no work. It is available to allow server response testing
//! and timing. It has no arguments and returns nothing.

use std::io::Write;

use tracing::debug;

use crate::protocol::xdr::{self, XDR};

/// Handles MOUNT protocol NULL procedure (procedure 0)
///
/// NULL is a no-operation RPC call used to check if the server is responding.
/// Takes no arguments and returns empty reply with successful status.
///
/// # Arguments
///
/// * `xid` - RPC transaction ID
/// * `output` - Output stream for writing the response
///
/// # Returns
///
/// * `Result<(), anyhow::Error>` - Ok(()) on success or an error
pub fn mountproc3_null(xid: u32, output: &mut impl Write) -> Result<(), anyhow::Error> {
    debug!("mountproc3_null({:?}) ", xid);
    // build an RPC reply
    let msg = xdr::rpc::make_success_reply(xid);
    debug!("\t{:?} --> {:?}", xid, msg);
    msg.serialize(output)?;
    Ok(())
}
