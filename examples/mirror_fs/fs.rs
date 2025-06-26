use std::ffi::OsStr;
use std::io::SeekFrom;
use std::ops::Bound;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::time::SystemTime;

use async_trait::async_trait;
use tokio::fs::{self, File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tracing::debug;

use nfs_mamont::fs_util::{file_setattr, metadata_to_fattr3, path_setattr};
use nfs_mamont::vfs;
use nfs_mamont::xdr::nfs3;

use crate::create_fs_object::CreateFSObject;
use crate::error_handling::{exists_no_traverse, NFSResult, RefreshResult};
use crate::fs_map::FSMap;

/// A file system implementation that mirrors a local directory
#[derive(Debug)]
pub struct MirrorFS {
    /// The file system map that tracks files and directories
    fsmap: tokio::sync::Mutex<FSMap>,
    generation: u64,
}

impl MirrorFS {
    /// Creates a new mirror file system with the given root path
    pub fn new(root: PathBuf) -> Self {
        let now = SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis();
        Self { fsmap: tokio::sync::Mutex::new(FSMap::new(root)), generation: now as u64 }
    }

    /// Creates a file system object in a given directory and of a given type
    /// Updates as much metadata as we can in-place
    async fn create_fs_object(
        &self,
        dirid: nfs3::fileid3,
        objectname: &nfs3::filename3,
        object: &CreateFSObject,
    ) -> NFSResult<(nfs3::fileid3, nfs3::fattr3)> {
        let mut fsmap = self.fsmap.lock().await;
        let ent = fsmap.find_entry(dirid)?;
        let mut path = fsmap.sym_to_path(&ent.name).await;
        let objectname_osstr = OsStr::from_bytes(objectname).to_os_string();
        path.push(&objectname_osstr);

        match object {
            CreateFSObject::Directory => {
                debug!("mkdir {:?}", path);
                if exists_no_traverse(&path) {
                    return Err(nfs3::nfsstat3::NFS3ERR_EXIST);
                }
                fs::create_dir(&path).await.map_err(|_| nfs3::nfsstat3::NFS3ERR_IO)?;
            }
            CreateFSObject::File(setattr) => {
                debug!("create {:?}", path);
                let file = std::fs::File::create(&path).map_err(|_| nfs3::nfsstat3::NFS3ERR_IO)?;
                let _ = file_setattr(&file, setattr).await;
            }
            CreateFSObject::Exclusive => {
                debug!("create exclusive {:?}", path);
                let _ = std::fs::File::options()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                    .map_err(|_| nfs3::nfsstat3::NFS3ERR_EXIST)?;
            }
            CreateFSObject::Symlink((_, target)) => {
                debug!("symlink {:?} {:?}", path, target);
                if exists_no_traverse(&path) {
                    return Err(nfs3::nfsstat3::NFS3ERR_EXIST);
                }
                fs::symlink(OsStr::from_bytes(target), &path)
                    .await
                    .map_err(|_| nfs3::nfsstat3::NFS3ERR_IO)?;
                // we do not set attributes on symlinks
            }
        }

        let _ = fsmap.refresh_entry(dirid).await?;

        let sym = fsmap.intern.intern(objectname_osstr).unwrap();
        let mut name = ent.name.clone();
        name.push(sym);
        let meta = path.symlink_metadata().map_err(|_| nfs3::nfsstat3::NFS3ERR_IO)?;
        let fileid = fsmap.create_entry(&name, meta.clone()).await;

        // update the children list
        if let Some(ref mut children) =
            fsmap.id_to_path.get_mut(&dirid).ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)?.children
        {
            children.insert(fileid);
        }
        Ok((fileid, metadata_to_fattr3(fileid, &meta)))
    }
}

#[async_trait]
impl vfs::NFSFileSystem for MirrorFS {
    fn generation(&self) -> u64 {
        self.generation
    }

    /// Returns the root directory file ID
    fn root_dir(&self) -> nfs3::fileid3 {
        0
    }

    /// Returns the capabilities of this file system
    fn capabilities(&self) -> vfs::Capabilities {
        vfs::Capabilities::ReadWrite
    }

    /// Looks up a file in a directory
    async fn lookup(
        &self,
        dirid: nfs3::fileid3,
        filename: &nfs3::filename3,
    ) -> NFSResult<nfs3::fileid3> {
        let mut fsmap = self.fsmap.lock().await;
        if let Ok(id) = fsmap.find_child(dirid, filename).await {
            if fsmap.id_to_path.contains_key(&id) {
                return Ok(id);
            }
        }
        // Optimize for negative lookups.
        // See if the file actually exists on the filesystem
        let dirent = fsmap.find_entry(dirid)?;
        let mut path = fsmap.sym_to_path(&dirent.name).await;
        let objectname_osstr = OsStr::from_bytes(filename).to_os_string();
        path.push(&objectname_osstr);
        if !exists_no_traverse(&path) {
            return Err(nfs3::nfsstat3::NFS3ERR_NOENT);
        }

        // The file exists on disk but not in our cache, so refresh the directory
        if let RefreshResult::Delete = fsmap.refresh_entry(dirid).await? {
            return Err(nfs3::nfsstat3::NFS3ERR_NOENT);
        }
        let _ = fsmap.refresh_dir_list(dirid).await?;

        fsmap.find_child(dirid, filename).await
    }

    /// Gets the attributes of a file
    async fn getattr(&self, id: nfs3::fileid3) -> NFSResult<nfs3::fattr3> {
        let mut fsmap = self.fsmap.lock().await;
        if let RefreshResult::Delete = fsmap.refresh_entry(id).await? {
            return Err(nfs3::nfsstat3::NFS3ERR_NOENT);
        }
        let ent = fsmap.find_entry(id)?;
        let path = fsmap.sym_to_path(&ent.name).await;
        debug!("Stat {:?}: {:?}", path, ent);
        Ok(ent.fsmeta)
    }

    /// Reads data from a file
    async fn read(&self, id: nfs3::fileid3, offset: u64, count: u32) -> NFSResult<(Vec<u8>, bool)> {
        let fsmap = self.fsmap.lock().await;
        let ent = fsmap.find_entry(id)?;
        let path = fsmap.sym_to_path(&ent.name).await;
        drop(fsmap);

        let mut f = File::open(&path).await.or(Err(nfs3::nfsstat3::NFS3ERR_NOENT))?;
        let len = f.metadata().await.or(Err(nfs3::nfsstat3::NFS3ERR_NOENT))?.len();
        let mut start = offset;
        let mut end = offset + count as u64;
        let eof = end >= len;
        if start >= len {
            start = len;
        }
        if end > len {
            end = len;
        }
        f.seek(SeekFrom::Start(start)).await.or(Err(nfs3::nfsstat3::NFS3ERR_IO))?;
        let mut buf = vec![0; (end - start) as usize];
        f.read_exact(&mut buf).await.or(Err(nfs3::nfsstat3::NFS3ERR_IO))?;
        Ok((buf, eof))
    }

    /// Reads directory entries
    async fn readdir(
        &self,
        dirid: nfs3::fileid3,
        start_after: nfs3::fileid3,
        max_entries: usize,
    ) -> NFSResult<vfs::ReadDirResult> {
        let mut fsmap = self.fsmap.lock().await;
        fsmap.refresh_entry(dirid).await?;
        fsmap.refresh_dir_list(dirid).await?;

        let entry = fsmap.find_entry(dirid)?;
        if !entry.is_directory() {
            return Err(nfs3::nfsstat3::NFS3ERR_NOTDIR);
        }
        debug!("readdir({:?}, {:?})", entry, start_after);
        // we must have children here
        let children = entry.children.ok_or(nfs3::nfsstat3::NFS3ERR_IO)?;

        let mut ret = vfs::ReadDirResult { entries: Vec::new(), end: false };

        let range_start =
            if start_after > 0 { Bound::Excluded(start_after) } else { Bound::Unbounded };

        let remaining_length = children.range((range_start, Bound::Unbounded)).count();
        let path = fsmap.sym_to_path(&entry.name).await;
        debug!("path: {:?}", path);
        debug!("children len: {:?}", children.len());
        debug!("remaining_len : {:?}", remaining_length);
        for i in children.range((range_start, Bound::Unbounded)) {
            let fileid = *i;
            let fileent = fsmap.find_entry(fileid)?;
            let name = fsmap.sym_to_fname(&fileent.name).await;
            debug!("\t --- {:?} {:?}", fileid, name);
            ret.entries.push(vfs::DirEntry {
                fileid,
                name: name.as_bytes().into(),
                attr: fileent.fsmeta,
            });
            if ret.entries.len() >= max_entries {
                break;
            }
        }
        if ret.entries.len() == remaining_length {
            ret.end = true;
        }
        debug!("readdir_result:{:?}", ret);

        Ok(ret)
    }

    /// Sets attributes of a file
    async fn setattr(&self, id: nfs3::fileid3, setattr: nfs3::sattr3) -> NFSResult<nfs3::fattr3> {
        let mut fsmap = self.fsmap.lock().await;
        let entry = fsmap.find_entry(id)?;
        let path = fsmap.sym_to_path(&entry.name).await;
        path_setattr(&path, &setattr).await?;

        // I have to lookup a second time to update
        let metadata = path.symlink_metadata().or(Err(nfs3::nfsstat3::NFS3ERR_IO))?;
        if let Ok(entry) = fsmap.find_entry_mut(id) {
            entry.fsmeta = metadata_to_fattr3(id, &metadata);
        }
        Ok(metadata_to_fattr3(id, &metadata))
    }

    /// Writes data to a file
    async fn write(&self, id: nfs3::fileid3, offset: u64, data: &[u8]) -> NFSResult<nfs3::fattr3> {
        let fsmap = self.fsmap.lock().await;
        let ent = fsmap.find_entry(id)?;
        let path = fsmap.sym_to_path(&ent.name).await;
        drop(fsmap);
        debug!("write to init {:?}", path);
        let mut f =
            OpenOptions::new().write(true).create(true).truncate(false).open(&path).await.map_err(
                |e| {
                    debug!("Unable to open {:?}", e);
                    nfs3::nfsstat3::NFS3ERR_IO
                },
            )?;
        f.seek(SeekFrom::Start(offset)).await.map_err(|e| {
            debug!("Unable to seek {:?}", e);
            nfs3::nfsstat3::NFS3ERR_IO
        })?;
        f.write_all(data).await.map_err(|e| {
            debug!("Unable to write {:?}", e);
            nfs3::nfsstat3::NFS3ERR_IO
        })?;
        debug!("write to {:?} {:?} {:?}", path, offset, data.len());
        let _ = f.flush().await;
        let _ = f.sync_all().await;
        let meta = f.metadata().await.or(Err(nfs3::nfsstat3::NFS3ERR_IO))?;
        Ok(metadata_to_fattr3(id, &meta))
    }

    /// Creates a file in a directory
    async fn create(
        &self,
        dirid: nfs3::fileid3,
        filename: &nfs3::filename3,
        setattr: nfs3::sattr3,
    ) -> NFSResult<(nfs3::fileid3, nfs3::fattr3)> {
        self.create_fs_object(dirid, filename, &CreateFSObject::File(setattr)).await
    }

    /// Creates an exclusive file in a directory
    async fn create_exclusive(
        &self,
        dirid: nfs3::fileid3,
        filename: &nfs3::filename3,
    ) -> NFSResult<nfs3::fileid3> {
        Ok(self.create_fs_object(dirid, filename, &CreateFSObject::Exclusive).await?.0)
    }

    /// Removes a file from a directory
    async fn remove(&self, dirid: nfs3::fileid3, filename: &nfs3::filename3) -> NFSResult<()> {
        let mut fsmap = self.fsmap.lock().await;
        let ent = fsmap.find_entry(dirid)?;
        let mut path = fsmap.sym_to_path(&ent.name).await;
        path.push(OsStr::from_bytes(filename));
        if let Ok(meta) = path.symlink_metadata() {
            if meta.is_dir() {
                fs::remove_dir(&path).await.map_err(|_| nfs3::nfsstat3::NFS3ERR_IO)?;
            } else {
                fs::remove_file(&path).await.map_err(|_| nfs3::nfsstat3::NFS3ERR_IO)?;
            }

            let filesym = fsmap.intern.intern(OsStr::from_bytes(filename).to_os_string()).unwrap();
            let mut sympath = ent.name.clone();
            sympath.push(filesym);
            if let Some(fileid) = fsmap.path_to_id.get(&sympath).copied() {
                // update the fileid -> path
                // and the path -> fileid mappings for the deleted file
                fsmap.id_to_path.remove(&fileid);
                fsmap.path_to_id.remove(&sympath);
                // we need to update the children listing for the directories
                if let Ok(dirent_mut) = fsmap.find_entry_mut(dirid) {
                    if let Some(ref mut fromch) = dirent_mut.children {
                        fromch.remove(&fileid);
                    }
                }
            }

            let _ = fsmap.refresh_entry(dirid).await;
        } else {
            return Err(nfs3::nfsstat3::NFS3ERR_NOENT);
        }

        Ok(())
    }

    /// Renames a file
    async fn rename(
        &self,
        from_dirid: nfs3::fileid3,
        from_filename: &nfs3::filename3,
        to_dirid: nfs3::fileid3,
        to_filename: &nfs3::filename3,
    ) -> NFSResult<()> {
        let mut fsmap = self.fsmap.lock().await;

        let from_dirent = fsmap.find_entry(from_dirid)?;
        let mut from_path = fsmap.sym_to_path(&from_dirent.name).await;
        from_path.push(OsStr::from_bytes(from_filename));

        let to_dirent = fsmap.find_entry(to_dirid)?;
        let mut to_path = fsmap.sym_to_path(&to_dirent.name).await;
        // to folder must exist
        if !exists_no_traverse(&to_path) {
            return Err(nfs3::nfsstat3::NFS3ERR_NOENT);
        }
        to_path.push(OsStr::from_bytes(to_filename));

        // src path must exist
        if !exists_no_traverse(&from_path) {
            return Err(nfs3::nfsstat3::NFS3ERR_NOENT);
        }
        debug!("Rename {:?} to {:?}", from_path, to_path);
        fs::rename(&from_path, &to_path).await.map_err(|_| nfs3::nfsstat3::NFS3ERR_IO)?;

        let oldsym = fsmap.intern.intern(OsStr::from_bytes(from_filename).to_os_string()).unwrap();
        let newsym = fsmap.intern.intern(OsStr::from_bytes(to_filename).to_os_string()).unwrap();

        let mut from_sympath = from_dirent.name.clone();
        from_sympath.push(oldsym);
        let mut to_sympath = to_dirent.name.clone();
        to_sympath.push(newsym);
        if let Some(fileid) = fsmap.path_to_id.get(&from_sympath).copied() {
            // update the fileid -> path
            // and the path -> fileid mappings for the new file
            fsmap.id_to_path.get_mut(&fileid).unwrap().name = to_sympath.clone();
            fsmap.path_to_id.remove(&from_sympath);
            fsmap.path_to_id.insert(to_sympath, fileid);
            if to_dirid != from_dirid {
                // moving across directories.
                // we need to update the children listing for the directories
                if let Ok(from_dirent_mut) = fsmap.find_entry_mut(from_dirid) {
                    if let Some(ref mut fromch) = from_dirent_mut.children {
                        fromch.remove(&fileid);
                    }
                }
                if let Ok(to_dirent_mut) = fsmap.find_entry_mut(to_dirid) {
                    if let Some(ref mut toch) = to_dirent_mut.children {
                        toch.insert(fileid);
                    }
                }
            }
        }
        let _ = fsmap.refresh_entry(from_dirid).await;
        if to_dirid != from_dirid {
            let _ = fsmap.refresh_entry(to_dirid).await;
        }

        Ok(())
    }

    /// Creates a directory
    async fn mkdir(
        &self,
        dirid: nfs3::fileid3,
        dirname: &nfs3::filename3,
    ) -> NFSResult<(nfs3::fileid3, nfs3::fattr3)> {
        self.create_fs_object(dirid, dirname, &CreateFSObject::Directory).await
    }

    /// Creates a symlink
    async fn symlink(
        &self,
        dirid: nfs3::fileid3,
        linkname: &nfs3::filename3,
        symlink: &nfs3::nfspath3,
        attr: &nfs3::sattr3,
    ) -> NFSResult<(nfs3::fileid3, nfs3::fattr3)> {
        self.create_fs_object(dirid, linkname, &CreateFSObject::Symlink((*attr, symlink.clone())))
            .await
    }

    /// Reads a symlink
    async fn readlink(&self, id: nfs3::fileid3) -> NFSResult<nfs3::nfspath3> {
        let fsmap = self.fsmap.lock().await;
        let ent = fsmap.find_entry(id)?;
        let path = fsmap.sym_to_path(&ent.name).await;
        drop(fsmap);
        if path.is_symlink() {
            if let Ok(target) = path.read_link() {
                Ok(target.as_os_str().as_bytes().into())
            } else {
                Err(nfs3::nfsstat3::NFS3ERR_IO)
            }
        } else {
            Err(nfs3::nfsstat3::NFS3ERR_BADTYPE)
        }
    }

    /// Creates a hard link
    async fn link(
        &self,
        file_id: nfs3::fileid3,
        link_dir_id: nfs3::fileid3,
        link_name: &nfs3::filename3,
    ) -> NFSResult<nfs3::fattr3> {
        let mut fsmap = self.fsmap.lock().await;

        // Get the source file entry
        let file_entry = fsmap.find_entry(file_id)?;
        let source_path = fsmap.sym_to_path(&file_entry.name).await;

        // Get the target directory entry
        let dir_entry = fsmap.find_entry(link_dir_id)?;
        let mut target_path = fsmap.sym_to_path(&dir_entry.name).await;
        let link_name_osstr = OsStr::from_bytes(link_name).to_os_string();
        target_path.push(&link_name_osstr);

        // Check if the target already exists
        if exists_no_traverse(&target_path) {
            return Err(nfs3::nfsstat3::NFS3ERR_EXIST);
        }

        // Create the hard link
        fs::hard_link(&source_path, &target_path).await.map_err(|_| nfs3::nfsstat3::NFS3ERR_IO)?;

        // Update the directory listing
        let sym = fsmap.intern.intern(link_name_osstr).unwrap();
        let mut name = dir_entry.name.clone();
        name.push(sym);
        let meta = target_path.symlink_metadata().map_err(|_| nfs3::nfsstat3::NFS3ERR_IO)?;
        let new_fileid = fsmap.create_entry(&name, meta.clone()).await;

        // Update the children list
        if let Some(ref mut children) =
            fsmap.id_to_path.get_mut(&link_dir_id).ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)?.children
        {
            children.insert(new_fileid);
        }

        // Return the attributes of the source file
        Ok(file_entry.fsmeta)
    }

    /// Creates a special file (device, socket, etc.)
    async fn mknod(
        &self,
        dir_id: nfs3::fileid3,
        name: &nfs3::filename3,
        ftype: nfs3::ftype3,
        _specdata: nfs3::specdata3,
        attrs: &nfs3::sattr3,
    ) -> NFSResult<(nfs3::fileid3, nfs3::fattr3)> {
        let mut fsmap = self.fsmap.lock().await;
        let dir_entry = fsmap.find_entry(dir_id)?;
        let mut path = fsmap.sym_to_path(&dir_entry.name).await;
        let name_osstr = OsStr::from_bytes(name).to_os_string();
        path.push(&name_osstr);

        // Check if the target already exists
        if exists_no_traverse(&path) {
            return Err(nfs3::nfsstat3::NFS3ERR_EXIST);
        }

        // Create the special file based on its type
        match ftype {
            nfs3::ftype3::NF3CHR => {
                // Character device
                let mode = match attrs.mode {
                    nfs3::set_mode3::Some(m) => m,
                    _ => 0o666,
                };

                // Create a regular file as a placeholder
                fs::OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .create(true)
                    .open(&path)
                    .await
                    .map_err(|_| nfs3::nfsstat3::NFS3ERR_IO)?;

                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode))
                        .map_err(|_| nfs3::nfsstat3::NFS3ERR_IO)?;

                    // Set ownership if provided
                    if let nfs3::set_uid3::Some(uid) = attrs.uid {
                        if let nfs3::set_gid3::Some(gid) = attrs.gid {
                            std::os::unix::fs::chown(&path, Some(uid), Some(gid))
                                .map_err(|_| nfs3::nfsstat3::NFS3ERR_IO)?;
                        }
                    }
                }
            }
            nfs3::ftype3::NF3BLK => {
                // Block device
                let mode = match attrs.mode {
                    nfs3::set_mode3::Some(m) => m,
                    _ => 0o666,
                };

                // Create a regular file as a placeholder
                fs::OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .create(true)
                    .open(&path)
                    .await
                    .map_err(|_| nfs3::nfsstat3::NFS3ERR_IO)?;

                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode))
                        .map_err(|_| nfs3::nfsstat3::NFS3ERR_IO)?;

                    // Set ownership if provided
                    if let nfs3::set_uid3::Some(uid) = attrs.uid {
                        if let nfs3::set_gid3::Some(gid) = attrs.gid {
                            std::os::unix::fs::chown(&path, Some(uid), Some(gid))
                                .map_err(|_| nfs3::nfsstat3::NFS3ERR_IO)?;
                        }
                    }
                }
            }
            nfs3::ftype3::NF3FIFO => {
                // Named pipe (FIFO)
                let mode = match attrs.mode {
                    nfs3::set_mode3::Some(m) => m,
                    _ => 0o666,
                };

                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    // Create a regular file as a placeholder since mkfifo is not available
                    fs::OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(true)
                        .open(&path)
                        .await
                        .map_err(|_| nfs3::nfsstat3::NFS3ERR_IO)?;

                    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode))
                        .map_err(|_| nfs3::nfsstat3::NFS3ERR_IO)?;

                    // Set ownership if provided
                    if let nfs3::set_uid3::Some(uid) = attrs.uid {
                        if let nfs3::set_gid3::Some(gid) = attrs.gid {
                            std::os::unix::fs::chown(&path, Some(uid), Some(gid))
                                .map_err(|_| nfs3::nfsstat3::NFS3ERR_IO)?;
                        }
                    }
                }
                #[cfg(not(unix))]
                {
                    return Err(nfsstat3::NFS3ERR_NOTSUPP);
                }
            }
            nfs3::ftype3::NF3SOCK => {
                // Socket
                let mode = match attrs.mode {
                    nfs3::set_mode3::Some(m) => m,
                    _ => 0o666,
                };

                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    // Create a regular file as a placeholder since mksock is not available
                    fs::OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(true)
                        .open(&path)
                        .await
                        .map_err(|_| nfs3::nfsstat3::NFS3ERR_IO)?;

                    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode))
                        .map_err(|_| nfs3::nfsstat3::NFS3ERR_IO)?;

                    // Set ownership if provided
                    if let nfs3::set_uid3::Some(uid) = attrs.uid {
                        if let nfs3::set_gid3::Some(gid) = attrs.gid {
                            std::os::unix::fs::chown(&path, Some(uid), Some(gid))
                                .map_err(|_| nfs3::nfsstat3::NFS3ERR_IO)?;
                        }
                    }
                }
                #[cfg(not(unix))]
                {
                    return Err(nfsstat3::NFS3ERR_NOTSUPP);
                }
            }
            _ => {
                return Err(nfs3::nfsstat3::NFS3ERR_BADTYPE);
            }
        }

        // Update the directory listing
        let sym = fsmap.intern.intern(name_osstr).unwrap();
        let mut full_name = dir_entry.name.clone();
        full_name.push(sym);
        let meta = path.symlink_metadata().map_err(|_| nfs3::nfsstat3::NFS3ERR_IO)?;
        let fileid = fsmap.create_entry(&full_name, meta.clone()).await;

        // Update the children list
        if let Some(ref mut children) =
            fsmap.id_to_path.get_mut(&dir_id).ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)?.children
        {
            children.insert(fileid);
        }

        // Return the file ID and attributes
        Ok((fileid, metadata_to_fattr3(fileid, &meta)))
    }

    /// Commits changes to a file
    async fn commit(
        &self,
        file_id: nfs3::fileid3,
        _offset: u64,
        _count: u32,
    ) -> NFSResult<nfs3::fattr3> {
        // For MirrorFS, we don't need to do anything special for commit
        // since we're already syncing the file after each write
        // Just return the current attributes
        self.getattr(file_id).await
    }
}
