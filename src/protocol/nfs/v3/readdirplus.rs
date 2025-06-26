//! Implementation of the READDIRPLUS procedure (procedure 17) for NFS version 3 protocol
//! as defined in RFC 1813 section 3.3.17.
//!
//! The READDIRPLUS procedure is an extended version of READDIR that returns
//! file handles and attributes in addition to directory entries. This procedure
//! is intended to eliminate separate LOOKUP calls for clients that want to
//! get attributes and file handles for directory entries.
//!
//! The client specifies:
//! - The file handle of the directory to read
//! - A cookie indicating where to start reading in the directory
//! - A cookie verifier to validate the cookie
//! - The maximum size of directory information to return
//! - The maximum size of attribute information to return
//!
//! On successful return, the server provides:
//! - The directory attributes
//! - A list of entries, each containing:
//!   * The file identifier (fileid)
//!   * The filename
//!   * A cookie for retrieving the next batch of entries
//!   * The file attributes
//!   * The file handle
//! - A flag indicating whether the end of the directory was reached

use std::io::{Read, Write};

use tracing::{debug, error, trace};

use crate::protocol::rpc;
use crate::protocol::xdr::{self, deserialize, nfs3, Serialize};

/// Handles NFSv3 READDIRPLUS procedure (procedure 17)
///
/// READDIRPLUS retrieves directory entries with their attributes and file handles.
/// Takes directory handle, cookie, cookie verifier, and maximum size limits.
/// Returns directory entries with file attributes and file handles for each entry.
///
/// # Arguments
///
/// * `xid` - RPC transaction ID
/// * `input` - Input stream containing the READDIRPLUS arguments
/// * `output` - Output stream for writing the response
/// * `context` - Server context containing VFS
///
/// # Returns
///
/// * `Result<(), anyhow::Error>` - Ok(()) on success or an error
pub async fn nfsproc3_readdirplus(
    xid: u32,
    input: &mut impl Read,
    output: &mut impl Write,
    context: &rpc::Context,
) -> Result<(), anyhow::Error> {
    let args = deserialize::<nfs3::dir::READDIRPLUS3args>(input)?;
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
    // initial call should hve empty cookie verf
    // subsequent calls should have cvf_version as defined above
    // which is based off the mtime.
    //
    // TODO: This is *far* too aggressive. and unnecessary.
    // The client should maintain this correctly typically.
    //
    // The way cookieverf is handled is quite interesting...
    //
    // There are 2 notes in the RFC of interest:
    // 1. If the
    // server detects that the cookie is no longer valid, the
    // server will reject the READDIR request with the status,
    // NFS3ERR_BAD_COOKIE. The client should be careful to
    // avoid holding directory entry cookies across operations
    // that modify the directory contents, such as REMOVE and
    // CREATE.
    //
    // 2. One implementation of the cookie-verifier mechanism might
    //  be for the server to use the modification time of the
    //  directory. This might be overly restrictive, however. A
    //  better approach would be to record the time of the last
    //  directory modification that changed the directory
    //  organization in a way that would make it impossible to
    //  reliably interpret a cookie. Servers in which directory
    //  cookies are always valid are free to use zero as the
    //  verifier always.
    //
    //  Basically, as long as the cookie is "kinda" intepretable,
    //  we should keep accepting it.
    //  On testing, the Mac NFS client pretty much expects that
    //  especially on highly concurrent modifications to the directory.
    //
    //  1. If part way through a directory enumeration we fail with BAD_COOKIE
    //  if the directory contents change, the client listing may fail resulting
    //  in a "no such file or directory" error.
    //  2. if we cache readdir results. i.e. we think of a readdir as two parts
    //     a. enumerating everything first
    //     b. the cookie is then used to paginate the enumeration
    //     we can run into file time synchronization issues. i.e. while one
    //     listing occurs and another file is touched, the listing may report
    //     an outdated file status.
    //
    //     This cache also appears to have to be *quite* long lasting
    //     as the client may hold on to a directory enumerator
    //     with unbounded time.
    //
    //  Basically, if we think about how linux directory listing works
    //  is that you just get an enumerator. There is no mechanic available for
    //  "restarting" a pagination and this enumerator is assumed to be valid
    //  even across directory modifications and should reflect changes
    //  immediately.
    //
    //  The best solution is simply to really completely avoid sending
    //  BAD_COOKIE all together and to ignore the cookie mechanism.
    //
    /*if args.cookieverf != nfs3::cookieverf3::default() && args.cookieverf != dirversion {
        info!(" -- Dir version mismatch. Received {:?}", args.cookieverf);
        make_success_reply(xid).serialize(output)?;
        nfs3::nfsstat3::NFS3ERR_BAD_COOKIE.serialize(output)?;
        dir_attr.serialize(output)?;
        return Ok(());
    }*/
    // subtract off the final entryplus* field (which must be false) and the eof
    let max_bytes_allowed = args.maxcount as usize - 128;
    // args.dircount is bytes of just fileid, name, cookie.
    // This is hard to ballpark, so we just divide it by 16
    let estimated_max_results = args.dircount / 16;
    let max_dircount_bytes = args.dircount as usize;
    let mut ctr = 0;
    match context.vfs.readdir(dirid, args.cookie, estimated_max_results as usize).await {
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
                let obj_attr = entry.attr;
                let handle = nfs3::post_op_fh3::Some(context.vfs.id_to_fh(entry.fileid));

                let entry = nfs3::dir::entryplus3 {
                    fileid: entry.fileid,
                    name: entry.name,
                    cookie: entry.fileid,
                    name_attributes: nfs3::post_op_attr::Some(obj_attr),
                    name_handle: handle,
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
                if added_output_bytes + counting_output.bytes_written() < max_bytes_allowed
                    && added_dircount + accumulated_dircount < max_dircount_bytes
                {
                    trace!("  -- dirent {:?}", entry);
                    // commit the entry
                    ctr += 1;
                    counting_output.write_all(&write_buf)?;
                    accumulated_dircount += added_dircount;
                    trace!(
                        "  -- lengths: {:?} / {:?} {:?} / {:?}",
                        accumulated_dircount,
                        max_dircount_bytes,
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
