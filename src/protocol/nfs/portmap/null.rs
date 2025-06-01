//! Implementation of the NULL procedure (procedure 0) for port mapper protocol
//! as defined in RFC 1057 A.2 section.
//! https://datatracker.ietf.org/doc/rfc1057/

use std::io::Write;

use tracing::debug;

use crate::protocol::xdr::{self, XDR};

/// Handles PMAPPROC_NULL procedure.
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
