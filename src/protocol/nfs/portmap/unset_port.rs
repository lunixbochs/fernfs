use std::io::{Read, Write};

use crate::protocol::nfs::portmap::PortmapKey;
use crate::protocol::rpc::Context;
use crate::xdr;
use crate::xdr::portmap::{mapping, IPPROTO_TCP, IPPROTO_UDP};
use crate::xdr::{deserialize, Serialize};

/// Removes port mappings for a given program and version from the portmap table.
///
/// This RPC procedure (`PMAPPROC_UNSET`) handles requests to unregister a program's ports
/// for both TCP and UDP protocols. It performs the following steps:
/// 1. Deserializes the input `mapping` (containing `prog`, `vers`, etc.).
/// 2. Attempts to remove entries for both TCP (`IPPROTO_TCP`) and UDP (`IPPROTO_UDP`).
/// 3. Returns an RPC success reply with a boolean indicating if any deletion occurred.
///
/// # Parameters
/// - `xid`: Transaction ID for RPC reply correlation.
/// - `read`: Input stream containing the XDR-serialized `mapping` (see `xdr::portmap::mapping`).
/// - `output`: Output stream for the XDR-serialized reply (success + deletion result).
/// - `context`: Shared NFS context holding the `portmap_table` (guarded by `RwLock`).
///
/// # Returns
/// - `Ok(())` on success, serializing:
///   - RPC success header (via `make_success_reply`).
///   - Boolean `true` if at least one port was removed, `false` otherwise.
/// - `Err(anyhow::Error)` on deserialization or serialization failures.
pub fn pmapproc_unsetport(
    xid: u32,
    read: &mut impl Read,
    output: &mut impl Write,
    context: &Context,
) -> Result<(), anyhow::Error> {
    let mapping = deserialize::<mapping>(read)?;
    let mut binding = context.portmap_table.write().unwrap();
    let tcp_removed = binding
        .table
        .remove(&PortmapKey { prog: mapping.prog, vers: mapping.vers, prot: IPPROTO_TCP })
        .is_some();
    let udp_removed = binding
        .table
        .remove(&PortmapKey { prog: mapping.prog, vers: mapping.vers, prot: IPPROTO_UDP })
        .is_some();
    let result = tcp_removed || udp_removed;
    xdr::rpc::make_success_reply(xid).serialize(output)?;
    result.serialize(output)?;
    Ok(())
}
