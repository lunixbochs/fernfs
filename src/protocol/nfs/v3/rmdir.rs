//! Implementation of the `RMDIR` procedure (procedure 13) for NFS version 3 protocol
//! as defined in RFC 1813 section 3.3.13.

use std::io::{Read, Write};

use tracing::{debug, error, warn};

use crate::protocol::rpc;
use crate::protocol::xdr::{self, deserialize, nfs3, Serialize};
use crate::vfs;

/// Handles `NFSv3` `RMDIR` procedure (procedure 13)
///
/// `RMDIR` removes a directory entry that must refer to a directory.
pub async fn nfsproc3_rmdir(
    xid: u32,
    input: &mut impl Read,
    output: &mut impl Write,
    context: &rpc::Context,
) -> Result<(), anyhow::Error> {
    if !matches!(context.vfs.capabilities(), vfs::Capabilities::ReadWrite) {
        warn!("No write capabilities.");
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        nfs3::nfsstat3::NFS3ERR_ROFS.serialize(output)?;
        nfs3::wcc_data::default().serialize(output)?;
        return Ok(());
    }

    let dirops = deserialize::<nfs3::diropargs3>(input)?;
    debug!("nfsproc3_rmdir({:?}, {:?}) ", xid, dirops);

    let dirid = context.vfs.fh_to_id(&dirops.dir);
    if let Err(stat) = dirid {
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        stat.serialize(output)?;
        nfs3::wcc_data::default().serialize(output)?;
        error!("Directory does not exist");
        return Ok(());
    }
    let dirid = dirid.unwrap();

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
    let pre_dir_attr = nfs3::pre_op_attr::Some(nfs3::wcc_attr {
        size: dir_attr.size,
        mtime: dir_attr.mtime,
        ctime: dir_attr.ctime,
    });
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

    let target_id = match context.vfs.lookup(dirid, &dirops.name).await {
        Ok(id) => id,
        Err(stat) => {
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            stat.serialize(output)?;
            let post_dir_attr = context.vfs.getattr(dirid).await.ok();
            nfs3::wcc_data { before: pre_dir_attr, after: post_dir_attr }.serialize(output)?;
            return Ok(());
        }
    };

    let target_attr = match context.vfs.getattr(target_id).await {
        Ok(attr) => attr,
        Err(stat) => {
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            stat.serialize(output)?;
            let post_dir_attr = context.vfs.getattr(dirid).await.ok();
            nfs3::wcc_data { before: pre_dir_attr, after: post_dir_attr }.serialize(output)?;
            return Ok(());
        }
    };

    if !matches!(target_attr.ftype, nfs3::ftype3::NF3DIR) {
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        nfs3::nfsstat3::NFS3ERR_NOTDIR.serialize(output)?;
        let post_dir_attr = context.vfs.getattr(dirid).await.ok();
        nfs3::wcc_data { before: pre_dir_attr, after: post_dir_attr }.serialize(output)?;
        return Ok(());
    }

    let res = context.vfs.remove(dirid, &dirops.name).await;
    let post_dir_attr = context.vfs.getattr(dirid).await.ok();
    let wcc_res = nfs3::wcc_data { before: pre_dir_attr, after: post_dir_attr };

    match res {
        Ok(()) => {
            debug!("rmdir success");
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            nfs3::nfsstat3::NFS3_OK.serialize(output)?;
            wcc_res.serialize(output)?;
        }
        Err(e) => {
            error!("rmdir error {:?} --> {:?}", xid, e);
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            e.serialize(output)?;
            wcc_res.serialize(output)?;
        }
    }

    Ok(())
}
