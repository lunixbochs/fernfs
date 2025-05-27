//! PORTMAP protocol implementation as specified in RFC 5531 (previously RFC 1057 Appendix A).
//!
//! The PORTMAP protocol (also known as Portmapper) is a service that maps RPC program numbers
//! to network port numbers. It is used by NFS clients to:
//! - Discover the port numbers for RPC services
//! - Dynamically locate services when port numbers change
//! - Register and unregister RPC services
//!
//! This module implements the following procedures of the PORTMAP protocol:
//!
//! 1. NULL - Do nothing (ping server)
//! 2. GETPORT - Get port number for an RPC service
//!
//! The other procedures (SET, UNSET, DUMP, CALLIT) are not implemented in this version.
//!
//! The PORTMAP protocol is essential for the operation of NFS and other RPC-based
//! services, as it provides the service discovery mechanism that allows clients
//! to find the appropriate port numbers for each service.

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
/// Dispatches PORTMAP protocol RPC calls to appropriate procedure handlers.
/// Validates protocol version and provides RPC service port mapping.
/// Used by clients to discover available services and their port numbers.
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
