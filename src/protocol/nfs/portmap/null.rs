//! Implementation of the NULL procedure (procedure 0) for PORTMAP protocol
//! as defined in RFC 5531 (previously RFC 1057 Appendix A).
//!
//! The NULL procedure does no work. It is available to allow server response testing
//! and timing. It has no arguments and returns nothing.
//!
//! This simple procedure is often used by clients to:
//! - Verify that the server is running and responding
//! - Measure network latency to the server
//! - Test RPC transport connectivity

use std::io::Write;

use tracing::debug;

use crate::protocol::xdr::{self, XDR};

/// Handles PORTMAP protocol NULL procedure (procedure 0)
///
/// NULL is a no-operation RPC call used to check if the portmapper is responding.
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
pub fn pmapproc_null(xid: u32, output: &mut impl Write) -> Result<(), anyhow::Error> {
    debug!("pmapproc_null({:?}) ", xid);
    // build an RPC reply
    let msg = xdr::rpc::make_success_reply(xid);
    debug!("\t{:?} --> {:?}", xid, msg);
    msg.serialize(output)?;
    Ok(())
}
