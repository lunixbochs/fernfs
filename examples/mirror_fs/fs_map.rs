use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ffi::{OsStr, OsString};
use std::fs::Metadata;
use std::os::unix::fs::MetadataExt;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct InodeKey {
    dev: u64,
    ino: u64,
}

impl InodeKey {
    fn from_meta(meta: &Metadata) -> Self {
        Self { dev: meta.dev(), ino: meta.ino() }
    }
}

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
    /// Mapping from inode keys to file ID
    inode_to_id: HashMap<InodeKey, nfs3::fileid3>,
    /// Mapping from file ID to inode keys
    id_to_inode: HashMap<nfs3::fileid3, InodeKey>,
}

impl FSMap {
    /// Creates a new file system map with the given root path
    pub fn new(root: PathBuf) -> Self {
        // create root entry
        let root_meta = root.metadata().unwrap();
        let root_entry = FSEntry::new(Vec::new(), metadata_to_fattr3(1, &root_meta));
        let root_inode = InodeKey::from_meta(&root_meta);

        Self {
            root,
            next_fileid: AtomicU64::new(1),
            intern: SymbolTable::new(),
            id_to_path: HashMap::from([(0, root_entry)]),
            path_to_id: HashMap::from([(Vec::new(), 0)]),
            inode_to_id: HashMap::from([(root_inode, 0)]),
            id_to_inode: HashMap::from([(0, root_inode)]),
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

    /// Removes a single path mapping and drops the entry if no aliases remain.
    pub fn remove_path(&mut self, path: &[Symbol]) -> Option<nfs3::fileid3> {
        let fileid = self.path_to_id.remove(path)?;
        if let Some(entry) = self.id_to_path.get_mut(&fileid) {
            entry.aliases.remove(path);
            if entry.name == path {
                if let Some(new_primary) = entry.aliases.iter().next().cloned() {
                    entry.name = new_primary;
                }
            }
            if entry.aliases.is_empty() {
                self.id_to_path.remove(&fileid);
                if let Some(inode) = self.id_to_inode.remove(&fileid) {
                    self.inode_to_id.remove(&inode);
                }
            }
        }
        Some(fileid)
    }

    /// Renames a path prefix for an entry subtree.
    pub fn rename_path_prefix(&mut self, from_prefix: &[Symbol], to_prefix: &[Symbol]) {
        if from_prefix == to_prefix {
            return;
        }

        let mut updates: Vec<(Vec<Symbol>, Vec<Symbol>, nfs3::fileid3)> = Vec::new();
        for (path, fileid) in self.path_to_id.iter() {
            if path.starts_with(from_prefix) {
                let mut new_path = to_prefix.to_vec();
                new_path.extend_from_slice(&path[from_prefix.len()..]);
                updates.push((path.clone(), new_path, *fileid));
            }
        }

        for (old_path, new_path, fileid) in updates {
            self.path_to_id.remove(&old_path);
            self.path_to_id.insert(new_path.clone(), fileid);
            if let Some(entry) = self.id_to_path.get_mut(&fileid) {
                if entry.aliases.remove(&old_path) {
                    entry.aliases.insert(new_path.clone());
                }
                if entry.name == old_path {
                    entry.name = new_path;
                }
            }
        }
    }

    /// Removes a directory entry subtree by path.
    pub fn remove_path_tree(&mut self, path: Vec<Symbol>, id: nfs3::fileid3) {
        let entry = match self.id_to_path.get(&id).cloned() {
            Some(entry) => entry,
            None => return,
        };
        if entry.is_directory() {
            if let Some(children) = entry.children {
                for (name, child_id) in children {
                    let mut child_path = path.clone();
                    child_path.push(name);
                    if let Some(child_entry) = self.id_to_path.get(&child_id) {
                        if child_entry.is_directory() {
                            self.remove_path_tree(child_path.clone(), child_id);
                        } else {
                            self.remove_path(&child_path);
                        }
                    } else {
                        self.remove_path(&child_path);
                    }
                }
            }
        }
        self.remove_path(&path);
    }

    /// Deletes an entry and all its children from the file system map
    pub fn delete_entry(&mut self, id: nfs3::fileid3) {
        if let Some(entry) = self.id_to_path.get(&id).cloned() {
            self.remove_path_tree(entry.name, id);
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
        let mut existing_aliases: BTreeSet<Vec<Symbol>> = BTreeSet::new();
        let mut primary: Option<Vec<Symbol>> = None;

        for alias in entry.aliases.iter() {
            let path = self.sym_to_path(alias).await;
            if exists_no_traverse(&path) {
                existing_aliases.insert(alias.clone());
                if primary.is_none() {
                    primary = Some(alias.clone());
                }
            } else {
                self.path_to_id.remove(alias);
            }
        }

        let primary = match primary {
            Some(primary) => primary,
            None => {
                self.delete_entry(id);
                debug!("Deleting entry A {:?}: {:?}. Ent: {:?}", id, entry.name, entry);
                return Ok(RefreshResult::Delete);
            }
        };

        if let Some(entry_mut) = self.id_to_path.get_mut(&id) {
            entry_mut.aliases = existing_aliases;
            entry_mut.name = primary.clone();
        }

        let path = self.sym_to_path(&primary).await;
        let meta = fs::symlink_metadata(&path).await.map_err(|_| nfs3::nfsstat3::NFS3ERR_IO)?;
        let inode_key = InodeKey::from_meta(&meta);
        if self.id_to_inode.get(&id).copied() != Some(inode_key) {
            self.delete_entry(id);
            debug!("Deleting entry B {:?}: {:?}. Ent: {:?}", id, path, entry);
            return Ok(RefreshResult::Delete);
        }
        self.id_to_inode.insert(id, inode_key);
        self.inode_to_id.insert(inode_key, id);
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
            debug!("Deleting entry C {:?}: {:?}. Ent: {:?}", id, path, entry);
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
        let mut new_children: BTreeMap<Symbol, u64> = BTreeMap::new();
        debug!("Relisting entry {:?}: {:?}. Ent: {:?}", id, path, entry);

        if let Ok(mut listing) = fs::read_dir(&path).await {
            while let Some(entry) =
                listing.next_entry().await.map_err(|_| nfs3::nfsstat3::NFS3ERR_IO)?
            {
                let sym = self.intern.intern(entry.file_name()).unwrap();
                cur_path.push(sym);
                let meta = entry.metadata().await.unwrap();
                let next_id = self.create_entry(&cur_path, meta).await;
                new_children.insert(sym, next_id);
                cur_path.pop();
            }

            if let Some(old_children) = entry.children {
                for (name_sym, _) in old_children {
                    if !new_children.contains_key(&name_sym) {
                        let mut old_path = entry.name.clone();
                        old_path.push(name_sym);
                        self.remove_path(&old_path);
                    }
                }
            }

            if let Some(entry_mut) = self.id_to_path.get_mut(&id) {
                entry_mut.children = Some(new_children);
                entry_mut.children_meta = entry_mut.fsmeta;
            }
        }

        Ok(())
    }

    /// Creates a new entry in the file system map
    pub async fn create_entry(&mut self, fullpath: &Vec<Symbol>, meta: Metadata) -> nfs3::fileid3 {
        if let Some(chid) = self.path_to_id.get(fullpath) {
            if let Some(chent) = self.id_to_path.get_mut(chid) {
                chent.fsmeta = metadata_to_fattr3(*chid, &meta);
            }
            return *chid;
        }

        let inode_key = InodeKey::from_meta(&meta);
        if let Some(existing_id) = self.inode_to_id.get(&inode_key).copied() {
            if let Some(entry) = self.id_to_path.get_mut(&existing_id) {
                entry.fsmeta = metadata_to_fattr3(existing_id, &meta);
                entry.aliases.insert(fullpath.clone());
            }
            self.path_to_id.insert(fullpath.clone(), existing_id);
            self.id_to_inode.entry(existing_id).or_insert(inode_key);
            return existing_id;
        }

        // path does not exist and inode is new
        let next_id = self.next_fileid.fetch_add(1, Ordering::Relaxed);
        let metafattr = metadata_to_fattr3(next_id, &meta);
        let new_entry = FSEntry::new(fullpath.clone(), metafattr);
        debug!("creating new entry {:?}: {:?}", next_id, meta);
        self.id_to_path.insert(next_id, new_entry);
        self.path_to_id.insert(fullpath.clone(), next_id);
        self.inode_to_id.insert(inode_key, next_id);
        self.id_to_inode.insert(next_id, inode_key);
        next_id
    }
}
