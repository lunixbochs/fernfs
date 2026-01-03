use std::collections::{BTreeMap, BTreeSet};

use fernfs::xdr::nfs3::{createverf3, fattr3, fileid3, ftype3};
use intaglio::Symbol;

/// A file system entry representing a file or directory
#[derive(Debug, Clone)]
pub struct FSEntry {
    /// The name of the entry as a list of symbols
    pub name: Vec<Symbol>,
    /// All path aliases for this entry
    pub aliases: BTreeSet<Vec<Symbol>>,
    /// The file attributes of the entry
    pub fsmeta: fattr3,
    /// Metadata when building the children list
    pub children_meta: fattr3,
    /// Optional map of child name symbol to file ID
    pub children: Option<BTreeMap<Symbol, fileid3>>,
    /// Optional verifier for exclusive creates
    pub exclusive_verifier: Option<createverf3>,
}

impl FSEntry {
    /// Creates a new file system entry
    pub fn new(name: Vec<Symbol>, fsmeta: fattr3) -> Self {
        let mut aliases = BTreeSet::new();
        aliases.insert(name.clone());
        Self {
            name,
            aliases,
            fsmeta,
            children_meta: fsmeta,
            children: None,
            exclusive_verifier: None,
        }
    }

    /// Checks if the entry is a directory
    pub fn is_directory(&self) -> bool {
        matches!(self.fsmeta.ftype, ftype3::NF3DIR)
    }

    /// Checks if the entry has children
    pub fn has_children(&self) -> bool {
        self.children.is_some()
    }

    /// Adds a child to the entry
    pub fn add_child(&mut self, name: Symbol, child_id: fileid3) {
        if let Some(ref mut children) = self.children {
            children.insert(name, child_id);
        } else {
            self.children = Some(BTreeMap::from([(name, child_id)]));
        }
    }

    /// Removes a child from the entry
    pub fn remove_child(&mut self, name: Symbol) {
        if let Some(ref mut children) = self.children {
            children.remove(&name);
        }
    }
}
