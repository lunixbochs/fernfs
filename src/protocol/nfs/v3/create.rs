//! Implementation of the CREATE procedure (procedure 8) for NFS version 3 protocol
//! as defined in RFC 1813 section 3.3.8.
//!
//! The CREATE procedure creates a regular file in a specified directory.
//! The client specifies:
//! - The file handle of the parent directory
//! - The name for the new file
//! - The method of creation (UNCHECKED, GUARDED, or EXCLUSIVE)
//! - The initial attributes for the new file (for UNCHECKED and GUARDED modes)
//! - A creation verifier (for EXCLUSIVE mode)
//!
//! The three creation methods are:
//! - UNCHECKED: Creates the file or updates attributes if it exists
//! - GUARDED: Creates the file only if it doesn't exist
//! - EXCLUSIVE: Creates the file only if it doesn't exist, using a unique verifier
//!
//! On successful return, the server provides:
//! - The file handle of the new file
//! - The attributes of the new file
//! - The attributes of the parent directory before and after the operation (weak cache consistency)

use std::io::{Read, Write};

use tracing::{debug, error, warn};

use crate::protocol::rpc;
use crate::protocol::xdr::{self, deserialize, nfs3, Deserialize, Serialize};
use crate::vfs;

/// Handles NFSv3 CREATE procedure (procedure 8)
///
/// CREATE creates a regular file in a specified directory.
/// It supports three modes: UNCHECKED, GUARDED, and EXCLUSIVE.
/// Returns file handle and attributes of the newly created file.
///
/// # Arguments
///
/// * `xid` - RPC transaction ID
/// * `input` - Input stream containing the CREATE arguments
/// * `output` - Output stream for writing the response
/// * `context` - Server context containing VFS
///
/// # Returns
///
/// * `Result<(), anyhow::Error>` - Ok(()) on success or an error
pub async fn nfsproc3_create(
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

    let dirops = deserialize::<nfs3::diropargs3>(input)?;
    let createhow = deserialize::<nfs3::createmode3>(input)?;

    debug!("nfsproc3_create({:?}, {:?}, {:?}) ", xid, dirops, createhow);

    // find the directory we are supposed to create the
    // new file in
    let dirid = context.vfs.fh_to_id(&dirops.dir);
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

    // get the object attributes before the write
    let pre_dir_attr = match context.vfs.getattr(dirid).await {
        Ok(v) => {
            let wccattr = nfs3::wcc_attr { size: v.size, mtime: v.mtime, ctime: v.ctime };
            nfs3::pre_op_attr::Some(wccattr)
        }
        Err(stat) => {
            error!("Cannot stat directory");
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            stat.serialize(output)?;
            nfs3::wcc_data::default().serialize(output)?;
            return Ok(());
        }
    };
    let mut target_attributes = nfs3::sattr3::default();

    match createhow {
        nfs3::createmode3::UNCHECKED => {
            target_attributes.deserialize(input)?;
            debug!("create unchecked {:?}", target_attributes);
        }
        nfs3::createmode3::GUARDED => {
            target_attributes.deserialize(input)?;
            debug!("create guarded {:?}", target_attributes);
            if context.vfs.lookup(dirid, &dirops.name).await.is_ok() {
                // file exists. Fail with NFS3ERR_EXIST.
                // Re-read dir attributes
                // for post op attr
                let post_dir_attr = context.vfs.getattr(dirid).await.ok();

                xdr::rpc::make_success_reply(xid).serialize(output)?;
                nfs3::nfsstat3::NFS3ERR_EXIST.serialize(output)?;
                nfs3::wcc_data { before: pre_dir_attr, after: post_dir_attr }.serialize(output)?;
                return Ok(());
            }
        }
        nfs3::createmode3::EXCLUSIVE => {
            debug!("create exclusive");
        }
    }

    let fid: Result<nfs3::fileid3, nfs3::nfsstat3>;
    let postopattr: nfs3::post_op_attr;
    // fill in the fid and post op attr here
    if matches!(createhow, nfs3::createmode3::EXCLUSIVE) {
        // the API for exclusive is very slightly different
        // We are not returning a post op attribute
        fid = context.vfs.create_exclusive(dirid, &dirops.name).await;
        postopattr = nfs3::post_op_attr::None;
    } else {
        // create!
        let res = context.vfs.create(dirid, &dirops.name, target_attributes).await;
        fid = res.map(|x| x.0);
        postopattr = res.map(|(_, fattr)| fattr).ok();
    }

    // Re-read dir attributes for post op attr
    let post_dir_attr = context.vfs.getattr(dirid).await.ok();
    let wcc_res = nfs3::wcc_data { before: pre_dir_attr, after: post_dir_attr };

    match fid {
        Ok(fid) => {
            debug!("create success --> {:?}, {:?}", fid, postopattr);
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            nfs3::nfsstat3::NFS3_OK.serialize(output)?;
            // serialize CREATE3resok
            let fh = context.vfs.id_to_fh(fid);
            nfs3::post_op_fh3::Some(fh).serialize(output)?;
            postopattr.serialize(output)?;
            wcc_res.serialize(output)?;
        }
        Err(e) => {
            error!("create error --> {:?}", e);
            // serialize CREATE3resfail
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            e.serialize(output)?;
            wcc_res.serialize(output)?;
        }
    }

    Ok(())
}
