use std::sync::Mutex;
use std::time::SystemTime;

use async_trait::async_trait;

use nfs_mamont::vfs;
use nfs_mamont::xdr::nfs3;

use crate::fs_contents::FSContents;
use crate::fs_entry::{make_dir, make_file, FSEntry};

/// Demo implementation of an NFS file system.
/// Provides a simple in-memory file system that supports basic NFS operations.
#[derive(Debug)]
pub struct DemoFS {
    /// Vector of all file system entries, protected by a mutex for concurrent access
    fs: Mutex<Vec<FSEntry>>,
    /// File ID of the root directory
    rootdir: nfs3::fileid3,
    generation: u64,
}

impl Default for DemoFS {
    /// Creates a new DemoFS with just the root directory.
    ///
    /// Initializes an empty file system with only the special entry at index 0
    /// and the root directory at index 1.
    fn default() -> DemoFS {
        // Create only the root directory without additional files and folders
        let entries = vec![
            make_file("", 0, 0, &[]), // fileid 0 is special
            make_dir(
                "/",
                1,      // current id. Must match position in entries
                1,      // parent id
                vec![], // empty list of children
            ),
        ];

        let now = SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis();
        DemoFS { fs: Mutex::new(entries), rootdir: 1, generation: now as u64 }
    }
}

// For this demo file system we let the handle just be the file
// there is only 1 file. a.txt.
/// Implementation of the NFSFileSystem trait for DemoFS.
/// Provides all required NFS operations for the demo file system.
#[async_trait]
impl vfs::NFSFileSystem for DemoFS {
    fn generation(&self) -> u64 {
        self.generation
    }

    /// Returns the file ID of the root directory.
    fn root_dir(&self) -> nfs3::fileid3 {
        self.rootdir
    }

    /// Returns the capabilities of this file system.
    /// This demo supports both read and write operations.
    fn capabilities(&self) -> vfs::Capabilities {
        vfs::Capabilities::ReadWrite
    }

    /// Writes data to a file at the specified offset.
    /// Resizes the file if needed and updates its attributes.
    async fn write(
        &self,
        id: nfs3::fileid3,
        offset: u64,
        data: &[u8],
    ) -> Result<nfs3::fattr3, nfs3::nfsstat3> {
        {
            let mut fs = self.fs.lock().unwrap();
            let mut fssize = fs[id as usize].attr.size;
            if let FSContents::File(bytes) = &mut fs[id as usize].contents {
                let offset = offset as usize;
                if offset + data.len() > bytes.len() {
                    bytes.resize(offset + data.len(), 0);
                    bytes[offset..].copy_from_slice(data);
                    fssize = bytes.len() as u64;
                }
            }
            fs[id as usize].attr.size = fssize;
            fs[id as usize].attr.used = fssize;
        }
        self.getattr(id).await
    }

    /// Creates a new file in the specified directory.
    /// Adds the new file to the parent directory's contents.
    async fn create(
        &self,
        dirid: nfs3::fileid3,
        filename: &nfs3::filename3,
        _attr: nfs3::sattr3,
    ) -> Result<(nfs3::fileid3, nfs3::fattr3), nfs3::nfsstat3> {
        let newid: nfs3::fileid3;
        {
            let mut fs = self.fs.lock().unwrap();
            newid = fs.len() as nfs3::fileid3;
            fs.push(make_file(std::str::from_utf8(filename).unwrap(), newid, dirid, "".as_bytes()));
            if let FSContents::Directory(dir) = &mut fs[dirid as usize].contents {
                dir.push(newid);
            }
        }
        Ok((newid, self.getattr(newid).await.unwrap()))
    }

    /// Creates a file exclusively (not supported in this demo).
    async fn create_exclusive(
        &self,
        _dirid: nfs3::fileid3,
        _filename: &nfs3::filename3,
    ) -> Result<nfs3::fileid3, nfs3::nfsstat3> {
        Err(nfs3::nfsstat3::NFS3ERR_NOTSUPP)
    }

    /// Looks up a file or directory by name within a directory.
    /// Handles special cases for '.' and '..' directory entries.
    async fn lookup(
        &self,
        dirid: nfs3::fileid3,
        filename: &nfs3::filename3,
    ) -> Result<nfs3::fileid3, nfs3::nfsstat3> {
        let fs = self.fs.lock().unwrap();
        let entry = fs.get(dirid as usize).ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)?;
        if let FSContents::File(_) = entry.contents {
            return Err(nfs3::nfsstat3::NFS3ERR_NOTDIR);
        } else if let FSContents::Directory(dir) = &entry.contents {
            // if looking for dir/. its the current directory
            if filename[..] == [b'.'] {
                return Ok(dirid);
            }
            // if looking for dir/.. its the parent directory
            if filename[..] == [b'.', b'.'] {
                return Ok(entry.parent);
            }
            for i in dir {
                if let Some(f) = fs.get(*i as usize) {
                    if f.name[..] == filename[..] {
                        return Ok(*i);
                    }
                }
            }
        }
        Err(nfs3::nfsstat3::NFS3ERR_NOENT)
    }

    /// Gets the attributes of a file system entry.
    async fn getattr(&self, id: nfs3::fileid3) -> Result<nfs3::fattr3, nfs3::nfsstat3> {
        let fs = self.fs.lock().unwrap();
        let entry = fs.get(id as usize).ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)?;
        Ok(entry.attr)
    }

    /// Sets attributes for a file system entry.
    /// Updates times, ownership, and file size as requested.
    async fn setattr(
        &self,
        id: nfs3::fileid3,
        setattr: nfs3::sattr3,
    ) -> Result<nfs3::fattr3, nfs3::nfsstat3> {
        let mut fs = self.fs.lock().unwrap();
        let entry = fs.get_mut(id as usize).ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)?;
        match setattr.atime {
            nfs3::set_atime::DONT_CHANGE => {}
            nfs3::set_atime::SET_TO_CLIENT_TIME(c) => {
                entry.attr.atime = c;
            }
            nfs3::set_atime::SET_TO_SERVER_TIME => {
                let d = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
                entry.attr.atime.seconds = d.as_secs() as u32;
                entry.attr.atime.nseconds = d.subsec_nanos();
            }
        };
        match setattr.mtime {
            nfs3::set_mtime::DONT_CHANGE => {}
            nfs3::set_mtime::SET_TO_CLIENT_TIME(c) => {
                entry.attr.mtime = c;
            }
            nfs3::set_mtime::SET_TO_SERVER_TIME => {
                let d = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
                entry.attr.mtime.seconds = d.as_secs() as u32;
                entry.attr.mtime.nseconds = d.subsec_nanos();
            }
        };
        match setattr.uid {
            nfs3::set_uid3::Some(u) => {
                entry.attr.uid = u;
            }
            nfs3::set_uid3::None => {}
        }
        match setattr.gid {
            nfs3::set_gid3::Some(u) => {
                entry.attr.gid = u;
            }
            nfs3::set_gid3::None => {}
        }
        match setattr.size {
            nfs3::set_size3::Some(s) => {
                entry.attr.size = s;
                entry.attr.used = s;
                if let FSContents::File(bytes) = &mut entry.contents {
                    bytes.resize(s as usize, 0);
                }
            }
            nfs3::set_size3::None => {}
        }
        Ok(entry.attr)
    }

    /// Reads data from a file at the specified offset.
    /// Returns the data and an EOF indicator.
    async fn read(
        &self,
        id: nfs3::fileid3,
        offset: u64,
        count: u32,
    ) -> Result<(Vec<u8>, bool), nfs3::nfsstat3> {
        let fs = self.fs.lock().unwrap();
        let entry = fs.get(id as usize).ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)?;
        if let FSContents::Directory(_) = entry.contents {
            return Err(nfs3::nfsstat3::NFS3ERR_ISDIR);
        } else if let FSContents::File(bytes) = &entry.contents {
            let mut start = offset as usize;
            let mut end = offset as usize + count as usize;
            let eof = end >= bytes.len();
            if start >= bytes.len() {
                start = bytes.len();
            }
            if end > bytes.len() {
                end = bytes.len();
            }
            return Ok((bytes[start..end].to_vec(), eof));
        }
        Err(nfs3::nfsstat3::NFS3ERR_NOENT)
    }

    /// Reads directory entries, starting after the specified entry ID.
    /// Returns a list of directory entries and an indicator if there are more entries.
    async fn readdir(
        &self,
        dirid: nfs3::fileid3,
        start_after: nfs3::fileid3,
        max_entries: usize,
    ) -> Result<vfs::ReadDirResult, nfs3::nfsstat3> {
        let fs = self.fs.lock().unwrap();
        let entry = fs.get(dirid as usize).ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)?;
        if let FSContents::File(_) = entry.contents {
            return Err(nfs3::nfsstat3::NFS3ERR_NOTDIR);
        } else if let FSContents::Directory(dir) = &entry.contents {
            let mut ret = vfs::ReadDirResult { entries: Vec::new(), end: false };
            let mut start_index = 0;
            if start_after > 0 {
                if let Some(pos) = dir.iter().position(|&r| r == start_after) {
                    start_index = pos + 1;
                } else {
                    return Err(nfs3::nfsstat3::NFS3ERR_BAD_COOKIE);
                }
            }
            let remaining_length = dir.len() - start_index;

            for i in dir[start_index..].iter() {
                ret.entries.push(vfs::DirEntry {
                    fileid: *i,
                    name: fs[(*i) as usize].name.clone(),
                    attr: fs[(*i) as usize].attr,
                });
                if ret.entries.len() >= max_entries {
                    break;
                }
            }
            if ret.entries.len() == remaining_length {
                ret.end = true;
            }
            return Ok(ret);
        }
        Err(nfs3::nfsstat3::NFS3ERR_NOENT)
    }

    /// Removes a file or empty directory from a directory.
    async fn remove(
        &self,
        dirid: nfs3::fileid3,
        filename: &nfs3::filename3,
    ) -> Result<(), nfs3::nfsstat3> {
        let mut fs = self.fs.lock().unwrap();
        let dir_entry = fs.get(dirid as usize).ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)?;

        if let FSContents::File(_) = dir_entry.contents {
            return Err(nfs3::nfsstat3::NFS3ERR_NOTDIR);
        }

        // Find the file in the directory
        let file_id = {
            if let FSContents::Directory(dir) = &fs[dirid as usize].contents {
                let mut file_id = None;
                for &id in dir {
                    if let Some(file) = fs.get(id as usize) {
                        if file.name[..] == filename[..] {
                            file_id = Some(id);
                            break;
                        }
                    }
                }
                file_id.ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)?
            } else {
                return Err(nfs3::nfsstat3::NFS3ERR_NOTDIR);
            }
        };

        // Check that it's not a directory
        if let FSContents::Directory(_) = fs[file_id as usize].contents {
            // If trying to remove a directory using remove, return an error
            return Err(nfs3::nfsstat3::NFS3ERR_ISDIR);
        }

        // Remove the file from the directory list
        if let FSContents::Directory(dir) = &mut fs[dirid as usize].contents {
            dir.retain(|&id| id != file_id);
        }

        // Mark the file as deleted (in a real FS, we would completely remove it)
        // In our simple implementation, we just clear the name and contents
        fs[file_id as usize].name = Vec::new().into();
        fs[file_id as usize].contents = FSContents::File(Vec::new());

        Ok(())
    }

    /// Renames a file or directory from one location to another.
    /// Handles various edge cases like moving between directories.
    async fn rename(
        &self,
        from_dirid: nfs3::fileid3,
        from_filename: &nfs3::filename3,
        to_dirid: nfs3::fileid3,
        to_filename: &nfs3::filename3,
    ) -> Result<(), nfs3::nfsstat3> {
        let mut fs = self.fs.lock().unwrap();

        // Find the file in the source directory
        let file_id = {
            if let FSContents::Directory(dir) = &fs[from_dirid as usize].contents {
                let mut file_id = None;
                for &id in dir {
                    if let Some(file) = fs.get(id as usize) {
                        if file.name[..] == from_filename[..] {
                            file_id = Some(id);
                            break;
                        }
                    }
                }
                file_id.ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)?
            } else {
                return Err(nfs3::nfsstat3::NFS3ERR_NOTDIR);
            }
        };

        // Check that the target directory exists
        if !fs
            .get(to_dirid as usize)
            .is_some_and(|entry| matches!(entry.contents, FSContents::Directory(_)))
        {
            return Err(nfs3::nfsstat3::NFS3ERR_NOTDIR);
        }

        // Find ID of the file to remove (if it exists)
        let to_remove_id = if let FSContents::Directory(dir) = &fs[to_dirid as usize].contents {
            let mut to_remove = None;
            for &id in dir {
                if let Some(file) = fs.get(id as usize) {
                    if file.name[..] == to_filename[..] {
                        to_remove = Some(id);
                        break;
                    }
                }
            }
            to_remove
        } else {
            None
        };

        // If the file exists, remove it from the directory
        if let Some(id) = to_remove_id {
            if let FSContents::Directory(dir) = &mut fs[to_dirid as usize].contents {
                dir.retain(|&x| x != id);
            }
        }

        // If this is a move between directories
        if from_dirid != to_dirid {
            // Remove from the old directory
            if let FSContents::Directory(dir) = &mut fs[from_dirid as usize].contents {
                dir.retain(|&id| id != file_id);
            }

            // Add to the new directory
            if let FSContents::Directory(dir) = &mut fs[to_dirid as usize].contents {
                dir.push(file_id);
            }

            // Update the file's parent
            fs[file_id as usize].parent = to_dirid;
        }

        // Update the file name
        fs[file_id as usize].name = to_filename.to_vec().into();

        Ok(())
    }

    /// Creates a new directory with the specified name.
    async fn mkdir(
        &self,
        dirid: nfs3::fileid3,
        dirname: &nfs3::filename3,
    ) -> Result<(nfs3::fileid3, nfs3::fattr3), nfs3::nfsstat3> {
        let mut fs = self.fs.lock().unwrap();

        // Check that the parent directory exists
        if !fs
            .get(dirid as usize)
            .is_some_and(|entry| matches!(entry.contents, FSContents::Directory(_)))
        {
            return Err(nfs3::nfsstat3::NFS3ERR_NOTDIR);
        }

        // Check that a directory with this name doesn't already exist
        if let FSContents::Directory(dir) = &fs[dirid as usize].contents {
            if dir
                .iter()
                .any(|&id| fs.get(id as usize).is_some_and(|file| file.name[..] == dirname[..]))
            {
                return Err(nfs3::nfsstat3::NFS3ERR_EXIST);
            }
        }

        // Create a new directory
        let newid = fs.len() as nfs3::fileid3;
        fs.push(make_dir(std::str::from_utf8(dirname).unwrap(), newid, dirid, Vec::new()));

        // Add the new directory to the parent
        if let FSContents::Directory(dir) = &mut fs[dirid as usize].contents {
            dir.push(newid);
        }

        // Update the parent directory's modification time
        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
        fs[dirid as usize].attr.mtime.seconds = now.as_secs() as u32;
        fs[dirid as usize].attr.mtime.nseconds = now.subsec_nanos();

        // Return the ID and attributes of the new directory
        Ok((newid, fs[newid as usize].attr))
    }

    /// Creates a symbolic link pointing to the specified path.
    async fn symlink(
        &self,
        dirid: nfs3::fileid3,
        linkname: &nfs3::filename3,
        symlink: &nfs3::nfspath3,
        _attr: &nfs3::sattr3,
    ) -> Result<(nfs3::fileid3, nfs3::fattr3), nfs3::nfsstat3> {
        // In our simple implementation, we'll just create a special file
        // with contents representing the path the link points to
        let mut fs = self.fs.lock().unwrap();

        // Check that the parent directory exists
        if !fs
            .get(dirid as usize)
            .is_some_and(|entry| matches!(entry.contents, FSContents::Directory(_)))
        {
            return Err(nfs3::nfsstat3::NFS3ERR_NOTDIR);
        }

        // Create a new file but mark its type as a symbolic link
        let newid = fs.len() as nfs3::fileid3;
        let mut entry = make_file(std::str::from_utf8(linkname).unwrap(), newid, dirid, symlink);

        // Change type to symbolic link
        entry.attr.ftype = nfs3::ftype3::NF3LNK;

        fs.push(entry);

        // Add the new file to the parent directory
        if let FSContents::Directory(dir) = &mut fs[dirid as usize].contents {
            dir.push(newid);
        }

        // Update the parent directory's modification time
        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
        fs[dirid as usize].attr.mtime.seconds = now.as_secs() as u32;
        fs[dirid as usize].attr.mtime.nseconds = now.subsec_nanos();

        // Return the ID and attributes of the new file
        Ok((newid, fs[newid as usize].attr))
    }

    /// Reads the target of a symbolic link.
    async fn readlink(&self, id: nfs3::fileid3) -> Result<nfs3::nfspath3, nfs3::nfsstat3> {
        let fs = self.fs.lock().unwrap();

        // Check that the file exists
        let entry = fs.get(id as usize).ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)?;

        // Use matching instead of comparing with ftype3::NF3LNK
        match entry.attr.ftype {
            nfs3::ftype3::NF3LNK => {
                // Get the symbolic link content
                if let FSContents::File(bytes) = &entry.contents {
                    // Convert Vec<u8> to nfspath3
                    return Ok(bytes.to_vec().into());
                }
                Err(nfs3::nfsstat3::NFS3ERR_INVAL)
            }
            _ => Err(nfs3::nfsstat3::NFS3ERR_INVAL),
        }
    }

    /// Creates a hard link to an existing file.
    async fn link(
        &self,
        file_id: nfs3::fileid3,
        target_dir_id: nfs3::fileid3,
        link_name: &nfs3::filename3,
    ) -> Result<nfs3::fattr3, nfs3::nfsstat3> {
        let mut fs = self.fs.lock().unwrap();

        // Check that the source file exists
        let source_file = fs.get(file_id as usize).ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)?;

        // Check that the source is a file, not a directory
        if let FSContents::Directory(_) = source_file.contents {
            return Err(nfs3::nfsstat3::NFS3ERR_ISDIR);
        }

        // Check that the target directory exists
        if !fs
            .get(target_dir_id as usize)
            .is_some_and(|entry| matches!(entry.contents, FSContents::Directory(_)))
        {
            return Err(nfs3::nfsstat3::NFS3ERR_NOTDIR);
        }

        // Check if a file with the same name already exists in the target directory
        if let FSContents::Directory(dir) = &fs[target_dir_id as usize].contents {
            if dir
                .iter()
                .any(|&id| fs.get(id as usize).is_some_and(|file| file.name[..] == link_name[..]))
            {
                return Err(nfs3::nfsstat3::NFS3ERR_EXIST);
            }
        }

        // Create a new entry for the hard link
        let newid = fs.len() as nfs3::fileid3;
        let mut new_entry = source_file.clone();
        new_entry.id = newid;
        new_entry.name = link_name.to_vec().into();
        new_entry.parent = target_dir_id;

        // Add the new entry to the filesystem
        fs.push(new_entry);

        // Add the new entry to the target directory
        if let FSContents::Directory(dir) = &mut fs[target_dir_id as usize].contents {
            dir.push(newid);
        }

        // Update the link count of the original file
        if let Some(entry) = fs.get_mut(file_id as usize) {
            entry.attr.nlink += 1;
        }

        // Return the attributes of the new link
        Ok(fs[newid as usize].attr)
    }

    /// Creates a special device node file.
    async fn mknod(
        &self,
        dir_id: nfs3::fileid3,
        name: &nfs3::filename3,
        type_: nfs3::ftype3,
        device_spec: nfs3::specdata3,
        attrs: &nfs3::sattr3,
    ) -> Result<(nfs3::fileid3, nfs3::fattr3), nfs3::nfsstat3> {
        let mut fs = self.fs.lock().unwrap();

        // Check that the parent directory exists
        if !fs
            .get(dir_id as usize)
            .is_some_and(|entry| matches!(entry.contents, FSContents::Directory(_)))
        {
            return Err(nfs3::nfsstat3::NFS3ERR_NOTDIR);
        }

        // Check if a file with the same name already exists
        if let FSContents::Directory(dir) = &fs[dir_id as usize].contents {
            if dir
                .iter()
                .any(|&id| fs.get(id as usize).is_some_and(|file| file.name[..] == name[..]))
            {
                return Err(nfs3::nfsstat3::NFS3ERR_EXIST);
            }
        }

        // Create a new entry based on the type
        let newid = fs.len() as nfs3::fileid3;
        let mut entry;

        match type_ {
            nfs3::ftype3::NF3REG => {
                // Regular file
                entry = make_file(std::str::from_utf8(name).unwrap(), newid, dir_id, &[]);
            }
            nfs3::ftype3::NF3DIR => {
                // Directory
                entry = make_dir(std::str::from_utf8(name).unwrap(), newid, dir_id, Vec::new());
            }
            nfs3::ftype3::NF3BLK | nfs3::ftype3::NF3CHR => {
                // Block or character device
                entry = make_file(std::str::from_utf8(name).unwrap(), newid, dir_id, &[]);
                entry.attr.ftype = type_;
                entry.attr.rdev = device_spec;
            }
            nfs3::ftype3::NF3FIFO => {
                // Named pipe
                entry = make_file(std::str::from_utf8(name).unwrap(), newid, dir_id, &[]);
                entry.attr.ftype = type_;
            }
            nfs3::ftype3::NF3SOCK => {
                // Socket
                entry = make_file(std::str::from_utf8(name).unwrap(), newid, dir_id, &[]);
                entry.attr.ftype = type_;
            }
            _ => {
                // Unsupported file type
                return Err(nfs3::nfsstat3::NFS3ERR_NOTSUPP);
            }
        }

        // Apply any additional attributes
        if let nfs3::set_mode3::Some(mode) = attrs.mode {
            entry.attr.mode = mode;
        }

        if let nfs3::set_uid3::Some(uid) = attrs.uid {
            entry.attr.uid = uid;
        }

        if let nfs3::set_gid3::Some(gid) = attrs.gid {
            entry.attr.gid = gid;
        }

        // Add the new entry to the filesystem
        fs.push(entry);

        // Add the new entry to the parent directory
        if let FSContents::Directory(dir) = &mut fs[dir_id as usize].contents {
            dir.push(newid);
        }

        // Update the parent directory's modification time
        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
        fs[dir_id as usize].attr.mtime.seconds = now.as_secs() as u32;
        fs[dir_id as usize].attr.mtime.nseconds = now.subsec_nanos();

        // Return the ID and attributes of the new entry
        Ok((newid, fs[newid as usize].attr))
    }

    /// Commits any pending writes to stable storage.
    /// In this in-memory implementation, it simply returns the current attributes.
    async fn commit(
        &self,
        id: nfs3::fileid3,
        _offset: u64,
        _count: u32,
    ) -> Result<nfs3::fattr3, nfs3::nfsstat3> {
        // In a real filesystem, this would ensure that the data written
        // to the file is committed to persistent storage.
        // For this demo, we'll just update the file's modification time
        // and return the attributes.

        let mut fs = self.fs.lock().unwrap();
        let entry = fs.get_mut(id as usize).ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)?;

        // Update the file's modification time
        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
        entry.attr.mtime.seconds = now.as_secs() as u32;
        entry.attr.mtime.nseconds = now.subsec_nanos();

        // Return the updated attributes
        Ok(entry.attr)
    }
}
