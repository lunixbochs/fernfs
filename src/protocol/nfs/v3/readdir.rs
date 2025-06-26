//! Implementation of the READDIR procedure (procedure 16) for NFS version 3 protocol
//! as defined in RFC 1813 section 3.3.16.
//!
//! The READDIR procedure retrieves a variable number of entries from a directory.
//! This procedure is used by clients to browse through a directory to discover
//! the filenames stored within.
//!
//! The client specifies:
//! - The file handle of the directory to read
//! - A cookie indicating where to start reading in the directory
//! - A cookie verifier to validate the cookie
//! - The maximum size of directory information to return
//!
//! On successful return, the server provides:
//! - The directory attributes
//! - A list of directory entries, each containing:
//!   * The file identifier (fileid)
//!   * The filename
//!   * A cookie for retrieving the next batch of entries
//! - A flag indicating whether the end of the directory was reached

use std::io::{Read, Write};

use tracing::{debug, error, trace};

use crate::protocol::rpc;
use crate::protocol::xdr::{self, deserialize, nfs3, Serialize};

/// Handles NFSv3 READDIR procedure (procedure 16)
///
/// READDIR retrieves a variable number of entries from a directory.
/// It takes directory handle, cookie, cookie verifier and directory count limit.
/// Returns directory entries including file ID, name and cookie for each entry.
///
/// # Arguments
///
/// * `xid` - RPC transaction ID
/// * `input` - Input stream containing the READDIR arguments
/// * `output` - Output stream for writing the response
/// * `context` - Server context containing VFS
///
/// # Returns
///
/// * `Result<(), anyhow::Error>` - Ok(()) on success or an error
pub async fn nfsproc3_readdir(
    xid: u32,
    input: &mut impl Read,
    output: &mut impl Write,
    context: &rpc::Context,
) -> Result<(), anyhow::Error> {
    let args = deserialize::<nfs3::dir::READDIR3args>(input)?;
    debug!("nfsproc3_readdirplus({:?},{:?}) ", xid, args);

    let dirid = context.vfs.fh_to_id(&args.dir);
    // fail if unable to convert file handle
    if let Err(stat) = dirid {
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        stat.serialize(output)?;
        nfs3::post_op_attr::None.serialize(output)?;
        return Ok(());
    }
    let dirid = dirid.unwrap();
    let dir_attr_maybe = context.vfs.getattr(dirid).await;

    let dir_attr = dir_attr_maybe.ok();

    let dirversion = if let Ok(ref dir_attr) = dir_attr_maybe {
        let cvf_version =
            ((dir_attr.mtime.seconds as u64) << 32) | (dir_attr.mtime.nseconds as u64);
        cvf_version.to_be_bytes()
    } else {
        nfs3::cookieverf3::default()
    };
    debug!(" -- Dir attr {:?}", dir_attr);
    debug!(" -- Dir version {:?}", dirversion);
    let has_version = args.cookieverf != nfs3::cookieverf3::default();
    // subtract off the final entryplus* field (which must be false) and the eof
    let max_bytes_allowed = args.dircount as usize - 128;
    // args.dircount is bytes of just fileid, name, cookie.
    // This is hard to ballpark, so we just divide it by 16
    let estimated_max_results = args.dircount / 16;
    let mut ctr = 0;
    match context.vfs.readdir_simple(dirid, estimated_max_results as usize).await {
        Ok(result) => {
            // we count dir_count seperately as it is just a subset of fields
            let mut accumulated_dircount: usize = 0;
            let mut all_entries_written = true;

            // this is a wrapper around a writer that also just counts the number of bytes
            // written
            let mut counting_output = crate::write_counter::WriteCounter::new(output);

            xdr::rpc::make_success_reply(xid).serialize(&mut counting_output)?;
            nfs3::nfsstat3::NFS3_OK.serialize(&mut counting_output)?;
            dir_attr.serialize(&mut counting_output)?;
            dirversion.serialize(&mut counting_output)?;
            for entry in result.entries {
                let entry = nfs3::dir::entry3 {
                    fileid: entry.fileid,
                    name: entry.name,
                    cookie: entry.fileid,
                };
                // write the entry into a buffer first
                let mut write_buf: Vec<u8> = Vec::new();
                let mut write_cursor = std::io::Cursor::new(&mut write_buf);
                // true flag for the entryplus3* to mark that this contains an entry
                true.serialize(&mut write_cursor)?;
                entry.serialize(&mut write_cursor)?;
                write_cursor.flush()?;
                let added_dircount = std::mem::size_of::<nfs3::fileid3>()                   // fileid
                                    + std::mem::size_of::<u32>() + entry.name.len()  // name
                                    + std::mem::size_of::<nfs3::cookie3>(); // cookie
                let added_output_bytes = write_buf.len();
                // check if we can write without hitting the limits
                if added_output_bytes + counting_output.bytes_written() < max_bytes_allowed {
                    trace!("  -- dirent {:?}", entry);
                    // commit the entry
                    ctr += 1;
                    counting_output.write_all(&write_buf)?;
                    accumulated_dircount += added_dircount;
                    trace!(
                        "  -- lengths: {:?} / {:?} / {:?}",
                        accumulated_dircount,
                        counting_output.bytes_written(),
                        max_bytes_allowed
                    );
                } else {
                    trace!(" -- insufficient space. truncating");
                    all_entries_written = false;
                    break;
                }
            }
            // false flag for the final entryplus* linked list
            false.serialize(&mut counting_output)?;
            // eof flag is only valid here if we wrote everything
            if all_entries_written {
                debug!("  -- readdir eof {:?}", result.end);
                result.end.serialize(&mut counting_output)?;
            } else {
                debug!("  -- readdir eof {:?}", false);
                false.serialize(&mut counting_output)?;
            }
            debug!(
                "readir {}, has_version {},  start at {}, flushing {} entries, complete {}",
                dirid, has_version, args.cookie, ctr, all_entries_written
            );
        }
        Err(stat) => {
            error!("readdir error {:?} --> {:?} ", xid, stat);
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            stat.serialize(output)?;
            dir_attr.serialize(output)?;
        }
    };
    Ok(())
}
