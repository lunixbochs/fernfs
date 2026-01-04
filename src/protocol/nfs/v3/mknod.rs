//! Implementation of the `MKNOD` procedure (procedure 11) for NFS version 3 protocol
//! as defined in RFC 1813 section 3.3.11.
//!
//! The `MKNOD` procedure creates a special file of a specified type. Special files
//! can be device files (character or block), FIFOs (named pipes), or sockets.
//!
//! The client specifies:
//! - The directory file handle where the special file should be created
//! - The name to be given to the special file
//! - The type of the special file to be created (block, character, socket, or FIFO)
//! - For block and character device files, the device number (major and minor numbers)
//! - Initial attributes for the new special file
//!
//! On successful return, the server provides:
//! - The file handle of the newly created special file
//! - The attributes of the newly created special file
//! - The attributes of the directory before and after the operation (weak cache consistency)
//!
//! This procedure is primarily used by Unix clients to create device files and
//! other special file types.

use std::io::{Read, Write};

use tracing::{debug, error, warn};

use crate::protocol::rpc;
use crate::protocol::xdr::{self, deserialize, nfs3, Serialize};
use crate::vfs;

/// Handles `NFSv3` `MKNOD` procedure (procedure 11)
///
/// `MKNOD` creates a special file (device, FIFO, or socket).
/// Takes directory handle, name, file type and device specifications.
/// Returns file handle and attributes of the newly created special file.
///
/// # Arguments
///
/// * `xid` - RPC transaction ID
/// * `input` - Input stream containing the `MKNOD` arguments
/// * `output` - Output stream for writing the response
/// * `context` - Server context containing VFS
///
/// # Returns
///
/// * `Result<(), anyhow::Error>` - Ok(()) on success or an error
pub async fn nfsproc3_mknod(
    xid: u32,
    input: &mut impl Read,
    output: &mut impl Write,
    context: &rpc::Context,
) -> Result<(), anyhow::Error> {
    // if we do not have write capabilities
    if !matches!(context.vfs.capabilities(), vfs::Capabilities::ReadWrite) {
        warn!("No write capabilities.");
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        nfs3::nfsstat3::NFS3ERR_ROFS.serialize(output)?;
        nfs3::wcc_data::default().serialize(output)?;
        return Ok(());
    }

    let args = deserialize::<nfs3::dir::MKNOD3args>(input)?;
    debug!("nfsproc3_mknod({:?}, {:?}) ", xid, args);

    // find the directory we are supposed to create the special file in
    let dirid = context.vfs.fh_to_id(&args.where_dir.dir);
    if let Err(stat) = dirid {
        // directory does not exist
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        stat.serialize(output)?;
        nfs3::wcc_data::default().serialize(output)?;
        error!("Directory does not exist");
        return Ok(());
    }
    // found the directory, get the attributes
    let dirid = dirid.unwrap();

    // get the object attributes before the operation
    let dir_attr = match context.vfs.getattr(dirid).await {
        Ok(v) => v,
        Err(stat) => {
            error!("Cannot stat directory");
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            stat.serialize(output)?;
            nfs3::wcc_data::default().serialize(output)?;
            return Ok(());
        }
    };
    let pre_dir_attr =
        nfs3::pre_op_attr::Some(nfs3::wcc_attr { size: dir_attr.size, mtime: dir_attr.mtime, ctime: dir_attr.ctime });
    match context.vfs.check_access(dirid, &context.auth, nfs3::ACCESS3_MODIFY).await {
        Ok(granted) if granted & nfs3::ACCESS3_MODIFY != 0 => {}
        Ok(_) => {
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            nfs3::nfsstat3::NFS3ERR_ACCES.serialize(output)?;
            nfs3::wcc_data {
                before: pre_dir_attr,
                after: nfs3::post_op_attr::Some(dir_attr),
            }
            .serialize(output)?;
            return Ok(());
        }
        Err(stat) => {
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            stat.serialize(output)?;
            nfs3::wcc_data {
                before: pre_dir_attr,
                after: nfs3::post_op_attr::Some(dir_attr),
            }
            .serialize(output)?;
            return Ok(());
        }
    }

    let pipe_attrs = args.what.pipe_attributes.unwrap_or_default();
    let (ftype, specdata, attrs) = match args.what.mknod_type {
        nfs3::ftype3::NF3CHR | nfs3::ftype3::NF3BLK => match args.what.device.as_ref() {
            Some(device) => (args.what.mknod_type, device.spec, &device.dev_attributes),
            None => {
                xdr::rpc::make_success_reply(xid).serialize(output)?;
                nfs3::nfsstat3::NFS3ERR_INVAL.serialize(output)?;
                let post_dir_attr = context.vfs.getattr(dirid).await.ok();
                nfs3::wcc_data { before: pre_dir_attr, after: post_dir_attr }.serialize(output)?;
                return Ok(());
            }
        },
        nfs3::ftype3::NF3SOCK | nfs3::ftype3::NF3FIFO => {
            (args.what.mknod_type, nfs3::specdata3::default(), &pipe_attrs)
        }
        _ => {
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            nfs3::nfsstat3::NFS3ERR_INVAL.serialize(output)?;
            let post_dir_attr = context.vfs.getattr(dirid).await.ok();
            nfs3::wcc_data { before: pre_dir_attr, after: post_dir_attr }.serialize(output)?;
            return Ok(());
        }
    };

    // Call VFS mknod method
    match context.vfs.mknod(dirid, &args.where_dir.name, ftype, specdata, attrs).await {
        Ok((fid, fattr)) => {
            debug!("nfsproc3_mknod success --> {:?}, {:?}", fid, fattr);

            // Get the directory attributes after the operation
            let post_dir_attr = context.vfs.getattr(dirid).await.ok();

            let wcc_res = nfs3::wcc_data { before: pre_dir_attr, after: post_dir_attr };

            xdr::rpc::make_success_reply(xid).serialize(output)?;
            nfs3::nfsstat3::NFS3_OK.serialize(output)?;
            // serialize MKNOD3resok
            let fh = context.vfs.id_to_fh(fid);
            nfs3::post_op_fh3::Some(fh).serialize(output)?;
            nfs3::post_op_attr::Some(fattr).serialize(output)?;
            wcc_res.serialize(output)?;
        }
        Err(stat) => {
            debug!("nfsproc3_mknod error --> {:?}", stat);

            // Get the directory attributes after the operation (unchanged)
            let post_dir_attr = context.vfs.getattr(dirid).await.ok();

            let wcc_res = nfs3::wcc_data { before: pre_dir_attr, after: post_dir_attr };

            xdr::rpc::make_success_reply(xid).serialize(output)?;
            stat.serialize(output)?;
            wcc_res.serialize(output)?;
        }
    }

    Ok(())
}
