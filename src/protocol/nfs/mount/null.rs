//! Implementation of the NULL procedure (procedure 0) for MOUNT version 3 protocol
//! as defined in RFC 1813 section 5.2.0
//! https://datatracker.ietf.org/doc/html/rfc1813#section-5.2.0

use std::io::Write;

use tracing::debug;

use crate::protocol::xdr::{self, XDR};

/// Handles MOUNTPROC3_NULL procedure.
///
/// Procedure NULL does not do any work. It is made available
/// to allow server response testing and timing.
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
