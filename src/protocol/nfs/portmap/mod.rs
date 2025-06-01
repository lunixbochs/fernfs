//! PORTMAP protocol implementation as specified in RFC 1057 A.1 and A.2 sections.
//! https://datatracker.ietf.org/doc/rfc1057/

use std::io::{Read, Write};

use num_traits::cast::FromPrimitive;
use tracing::error;

use crate::protocol::rpc;
use crate::protocol::xdr::{self, portmap, XDR};

mod get_port;
mod null;

use get_port::pmapproc_getport;
use null::pmapproc_null;

/// Main handler for PORTMAP protocol
///
/// TODO: Unimplemented procedures:
/// * PMAPPROC_SET - Register a new port mapping
/// * PMAPPROC_UNSET - Remove a port mapping
/// * PMAPPROC_DUMP - List all registered port mappings
/// * PMAPPROC_CALLIT - Forward a call to another RPC service
///
/// # Arguments
///
/// * `xid` - RPC transaction ID from the client
/// * `call` - The RPC call body containing program, version, and procedure numbers
/// * `input` - Input stream for reading procedure arguments
/// * `output` - Output stream for writing procedure results
/// * `context` - Server context containing port information
///
/// # Returns
///
/// * `Result<(), anyhow::Error>` - Ok(()) on success or an error
pub fn handle_portmap(
    xid: u32,
    call: xdr::rpc::call_body,
    input: &mut impl Read,
    output: &mut impl Write,
    context: &rpc::Context,
) -> Result<(), anyhow::Error> {
    if call.vers != portmap::VERSION {
        error!("Invalid Portmap Version number {} != {}", call.vers, portmap::VERSION);
        xdr::rpc::prog_mismatch_reply_message(xid, portmap::VERSION).serialize(output)?;
        return Ok(());
    }
    let prog =
        portmap::PortmapProgram::from_u32(call.proc).unwrap_or(portmap::PortmapProgram::INVALID);

    match prog {
        portmap::PortmapProgram::PMAPPROC_NULL => pmapproc_null(xid, output)?,
        portmap::PortmapProgram::PMAPPROC_GETPORT => pmapproc_getport(xid, input, output, context)?,
        _ => {
            xdr::rpc::proc_unavail_reply_message(xid).serialize(output)?;
        }
    }
    Ok(())
}
