use std::collections::{BTreeSet, HashMap};
use std::ffi::{OsStr, OsString};
use std::fs::Metadata;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use intaglio::osstr::SymbolTable;
use intaglio::Symbol;
use tokio::fs;
use tracing::debug;

use nfs_mamont::fs_util::*;
use nfs_mamont::xdr::nfs3;

use crate::error_handling::{exists_no_traverse, NFSResult, RefreshResult};
use crate::fs_entry::FSEntry;

/// A file system mapping structure that maintains the relationship between file IDs and paths
#[derive(Debug)]
pub struct FSMap {
    /// The root directory path
    pub root: PathBuf,
    /// The next available file ID
    pub next_fileid: AtomicU64,
    /// Symbol table for string internment
    pub intern: SymbolTable,
    /// Mapping from file ID to file system entry
    pub id_to_path: HashMap<nfs3::fileid3, FSEntry>,
    /// Mapping from path symbols to file ID
    pub path_to_id: HashMap<Vec<Symbol>, nfs3::fileid3>,
}

impl FSMap {
    /// Creates a new file system map with the given root path
    pub fn new(root: PathBuf) -> Self {
        // create root entry
        let root_entry = FSEntry::new(Vec::new(), metadata_to_fattr3(1, &root.metadata().unwrap()));

        Self {
            root,
            next_fileid: AtomicU64::new(1),
            intern: SymbolTable::new(),
            id_to_path: HashMap::from([(0, root_entry)]),
            path_to_id: HashMap::from([(Vec::new(), 0)]),
        }
    }

    /// Converts a list of symbols to a full path
    pub async fn sym_to_path(&self, symlist: &[Symbol]) -> PathBuf {
        let mut ret = self.root.clone();
        for i in symlist.iter() {
            ret.push(self.intern.get(*i).unwrap());
        }
        ret
    }

    /// Converts a list of symbols to a file name
    pub async fn sym_to_fname(&self, symlist: &[Symbol]) -> OsString {
        if let Some(x) = symlist.last() {
            self.intern.get(*x).unwrap().into()
        } else {
            "".into()
        }
    }

    /// Collects all children of a given file ID recursively
    pub fn collect_all_children(&self, id: nfs3::fileid3, ret: &mut Vec<nfs3::fileid3>) {
        ret.push(id);
        if let Some(entry) = self.id_to_path.get(&id) {
            if let Some(ref ch) = entry.children {
                for i in ch.iter() {
                    self.collect_all_children(*i, ret);
                }
            }
        }
    }

    /// Deletes an entry and all its children from the file system map
    pub fn delete_entry(&mut self, id: nfs3::fileid3) {
        let mut children = Vec::new();
        self.collect_all_children(id, &mut children);
        for i in children.iter() {
            if let Some(ent) = self.id_to_path.remove(i) {
                self.path_to_id.remove(&ent.name);
            }
        }
    }

    /// Finds an entry by its file ID
    pub fn find_entry(&self, id: nfs3::fileid3) -> NFSResult<FSEntry> {
        Ok(self.id_to_path.get(&id).ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)?.clone())
    }

    /// Finds a mutable entry by its file ID
    pub fn find_entry_mut(&mut self, id: nfs3::fileid3) -> NFSResult<&mut FSEntry> {
        self.id_to_path.get_mut(&id).ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)
    }

    /// Finds a child entry by its parent ID and filename
    pub async fn find_child(&self, id: nfs3::fileid3, filename: &[u8]) -> NFSResult<nfs3::fileid3> {
        let mut name = self.id_to_path.get(&id).ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)?.name.clone();
        name.push(
            self.intern
                .check_interned(OsStr::from_bytes(filename))
                .ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)?,
        );
        Ok(*self.path_to_id.get(&name).ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)?)
    }

    /// Refreshes an entry by checking if it still exists and updating its metadata
    pub async fn refresh_entry(&mut self, id: nfs3::fileid3) -> NFSResult<RefreshResult> {
        let entry = self.id_to_path.get(&id).ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)?.clone();
        let path = self.sym_to_path(&entry.name).await;

        if !exists_no_traverse(&path) {
            self.delete_entry(id);
            debug!("Deleting entry A {:?}: {:?}. Ent: {:?}", id, path, entry);
            return Ok(RefreshResult::Delete);
        }

        let meta = fs::symlink_metadata(&path).await.map_err(|_| nfs3::nfsstat3::NFS3ERR_IO)?;
        let meta = metadata_to_fattr3(id, &meta);
        if !fattr3_differ(&meta, &entry.fsmeta) {
            return Ok(RefreshResult::Noop);
        }

        // If we get here we have modifications
        if entry.fsmeta.ftype as u32 != meta.ftype as u32 {
            // if the file type changed ex: file->dir or dir->file
            // really the entire file has been replaced.
            // we expire the entire id
            debug!("File Type Mismatch FT {:?} : {:?} vs {:?}", id, entry.fsmeta.ftype, meta.ftype);
            debug!("File Type Mismatch META {:?} : {:?} vs {:?}", id, entry.fsmeta, meta);
            self.delete_entry(id);
            debug!("Deleting entry B {:?}: {:?}. Ent: {:?}", id, path, entry);
            return Ok(RefreshResult::Delete);
        }

        // inplace modification.
        // update metadata
        self.id_to_path.get_mut(&id).unwrap().fsmeta = meta;
        debug!("Reloading entry {:?}: {:?}. Ent: {:?}", id, path, entry);
        Ok(RefreshResult::Reload)
    }

    /// Refreshes the directory listing for a given directory ID
    pub async fn refresh_dir_list(&mut self, id: nfs3::fileid3) -> NFSResult<()> {
        let entry = self.id_to_path.get(&id).ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)?.clone();

        // if there are children and the metadata did not change
        if entry.children.is_some() && !fattr3_differ(&entry.children_meta, &entry.fsmeta) {
            return Ok(());
        }

        if !entry.is_directory() {
            return Ok(());
        }

        let mut cur_path = entry.name.clone();
        let path = self.sym_to_path(&entry.name).await;
        let mut new_children: Vec<u64> = Vec::new();
        debug!("Relisting entry {:?}: {:?}. Ent: {:?}", id, path, entry);

        if let Ok(mut listing) = fs::read_dir(&path).await {
            while let Some(entry) =
                listing.next_entry().await.map_err(|_| nfs3::nfsstat3::NFS3ERR_IO)?
            {
                let sym = self.intern.intern(entry.file_name()).unwrap();
                cur_path.push(sym);
                let meta = entry.metadata().await.unwrap();
                let next_id = self.create_entry(&cur_path, meta).await;
                new_children.push(next_id);
                cur_path.pop();
            }

            self.id_to_path.get_mut(&id).ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)?.children =
                Some(BTreeSet::from_iter(new_children.into_iter()));
        }

        Ok(())
    }

    /// Creates a new entry in the file system map
    pub async fn create_entry(&mut self, fullpath: &Vec<Symbol>, meta: Metadata) -> nfs3::fileid3 {
        let next_id = if let Some(chid) = self.path_to_id.get(fullpath) {
            if let Some(chent) = self.id_to_path.get_mut(chid) {
                chent.fsmeta = metadata_to_fattr3(*chid, &meta);
            }
            *chid
        } else {
            // path does not exist
            let next_id = self.next_fileid.fetch_add(1, Ordering::Relaxed);
            let metafattr = metadata_to_fattr3(next_id, &meta);
            let new_entry = FSEntry::new(fullpath.clone(), metafattr);
            debug!("creating new entry {:?}: {:?}", next_id, meta);
            self.id_to_path.insert(next_id, new_entry);
            self.path_to_id.insert(fullpath.clone(), next_id);
            next_id
        };
        next_id
    }
}
