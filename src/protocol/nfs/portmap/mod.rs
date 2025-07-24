//! `PORTMAP` protocol implementation as specified in RFC 1057 A.1 and A.2 sections.
//! <https://datatracker.ietf.org/doc/rfc1057/>.

use std::collections::HashMap;
use std::io::{Read, Write};

use num_traits::cast::FromPrimitive;
use tracing::error;

use crate::protocol::xdr::{self, portmap, Serialize};

mod get_port;
mod null;
mod set_port;
mod unset_port;

use get_port::pmapproc_getport;
use null::pmapproc_null;

use crate::protocol::nfs::portmap::set_port::pmapproc_setport;
use crate::protocol::nfs::portmap::unset_port::pmapproc_unsetport;
use crate::protocol::rpc::Context;

///Stores mapping program to port
#[derive(Default)]
pub struct PortmapTable {
    table: HashMap<PortmapKey, u16>,
}
///Represents entry of PortmapTable
#[derive(Debug, Hash, Eq, PartialEq)]
pub struct PortmapKey {
    /// The program number
    prog: u32,
    /// The program version number
    vers: u32,
    /// The transport protocol
    prot: u32,
}

/// Main handler for PORTMAP protocol
///
/// TODO: Unimplemented procedures:
/// * `PMAPPROC_UNSET` - Remove a port mapping
/// * `PMAPPROC_DUMP` - List all registered port mappings
/// * `PMAPPROC_CALLIT` - Forward a call to another RPC service
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
    call: &xdr::rpc::call_body,
    input: &mut impl Read,
    output: &mut impl Write,
    context: &mut Context,
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
        portmap::PortmapProgram::PMAPPROC_SET => pmapproc_setport(xid, input, output, context)?,
        portmap::PortmapProgram::PMAPPROC_UNSET => pmapproc_unsetport(xid, input, output, context)?,
        _ => {
            xdr::rpc::proc_unavail_reply_message(xid).serialize(output)?;
        }
    }
    Ok(())
}

/// Looks up a port in the Portmap table using the specified entry
fn get_port(context: &Context, entry: &PortmapKey) -> Option<u16> {
    let binding = context.portmap_table.read().unwrap();
    binding.table.get(entry).copied()
}
