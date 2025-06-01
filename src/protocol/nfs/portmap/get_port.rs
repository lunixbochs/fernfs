//! Implementation of the GETPORT procedure (procedure 3) for port mapper protocol
//! as defined in RFC 1057 A.2 section.
//! https://datatracker.ietf.org/doc/rfc1057/

use std::io::{Read, Write};

use tracing::debug;

use crate::protocol::rpc;
use crate::protocol::xdr::{self, XDR};

/// Handles PMAPPROC_GETPORT procedure.
///
/// GETPORT maps an RPC program and version to a TCP/UDP port.
/// Takes a mapping request with program number, version, protocol and port.
/// Returns the port number where the requested service can be reached.
///
/// TODO: Function always returns the same host port and ignores the
/// requested version and protocol (always TCP). In the future, proper program
/// to port mapping should be implemented.
///
/// # Arguments
///
/// * `xid` - RPC transaction ID
/// * `read` - Input stream containing the port mapping request
/// * `output` - Output stream for writing the response
/// * `context` - Server context containing port information
///
/// # Returns
///
/// * `Result<(), anyhow::Error>` - Ok(()) on success or an error
pub fn pmapproc_getport(
    xid: u32,
    read: &mut impl Read,
    output: &mut impl Write,
    context: &rpc::Context,
) -> Result<(), anyhow::Error> {
    let mut mapping = xdr::portmap::mapping::default();
    mapping.deserialize(read)?;
    debug!("pmapproc_getport({:?}, {:?}) ", xid, mapping);
    xdr::rpc::make_success_reply(xid).serialize(output)?;
    let port = context.local_port as u32;
    debug!("\t{:?} --> {:?}", xid, port);
    port.serialize(output)?;
    Ok(())
}
