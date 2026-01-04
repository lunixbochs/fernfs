//! Implementation of the MNT procedure (procedure 1) for `MOUNT` version 3 protocol
//! as defined in RFC 1813 section 5.2.1.
//! <https://datatracker.ietf.org/doc/html/rfc1813#section-5.2.1>.

use std::io::{Read, Write};

use num_traits::cast::ToPrimitive;
use tracing::debug;

use crate::protocol::rpc;
use crate::protocol::xdr::{self, deserialize, mount, Serialize};

fn trim_start_slashes(mut path: &[u8]) -> &[u8] {
    while path.first() == Some(&b'/') {
        path = &path[1..];
    }
    path
}

fn trim_end_slashes(mut path: &[u8]) -> &[u8] {
    while path.last() == Some(&b'/') {
        path = &path[..path.len() - 1];
    }
    path
}

fn trim_slashes(path: &[u8]) -> &[u8] {
    trim_end_slashes(trim_start_slashes(path))
}

fn export_subpath<'a>(requested: &'a [u8], export: &[u8]) -> Option<&'a [u8]> {
    if export == b"/" {
        if !requested.starts_with(b"/") {
            return None;
        }
        return Some(&requested[1..]);
    }

    let rest = requested.strip_prefix(export)?;
    if rest.is_empty() {
        return Some(rest);
    }
    if rest.first() == Some(&b'/') {
        return Some(&rest[1..]);
    }
    None
}

/// Handles `MOUNTPROC3_MNT` procedure.
///
/// Function returns file handle for the requested
/// mount point and supported authentication flavors.
///
/// TODO: Currently there is only one mount point, to support
/// full functionality we need to extend support for multiple mount points.
///
/// # Arguments
///
/// * `xid` - RPC transaction ID
/// * `input` - Input stream containing the directory path to mount
/// * `output` - Output stream for writing the response
/// * `context` - Server context containing exports and VFS information
///
/// # Returns
///
/// * `Result<(), anyhow::Error>` - Ok(()) on success or an error
pub async fn mountproc3_mnt(
    xid: u32,
    input: &mut impl Read,
    output: &mut impl Write,
    context: &rpc::Context,
) -> Result<(), anyhow::Error> {
    let path = deserialize::<Vec<u8>>(input)?;
    let path_display = String::from_utf8_lossy(&path);
    debug!("mountproc3_mnt({:?},{:?}) ", xid, path_display);
    let export = context.export_name.as_bytes();
    let path = if let Some(path) = export_subpath(&path, export) {
        let path = trim_slashes(path);
        let mut new_path = Vec::with_capacity(path.len() + 1);
        new_path.push(b'/');
        new_path.extend_from_slice(path);
        new_path
    } else {
        // invalid export
        debug!("{:?} --> no matching export", xid);
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        mount::mountstat3::MNT3ERR_NOENT.serialize(output)?;
        return Ok(());
    };
    if let Ok(fileid) = context.vfs.path_to_id(&path).await {
        let response = mount::mountres3_ok {
            fhandle: context.vfs.id_to_fh(fileid).data,
            auth_flavors: vec![
                xdr::rpc::auth_flavor::AUTH_NULL.to_u32().unwrap(),
                xdr::rpc::auth_flavor::AUTH_UNIX.to_u32().unwrap(),
            ],
        };
        debug!("{:?} --> {:?}", xid, response);
        if let Some(ref chan) = context.mount_signal {
            let _ = chan.send(true).await;
        }
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        mount::mountstat3::MNT3_OK.serialize(output)?;
        response.serialize(output)?;
    } else {
        debug!("{:?} --> MNT3ERR_NOENT", xid);
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        mount::mountstat3::MNT3ERR_NOENT.serialize(output)?;
    }
    Ok(())
}
