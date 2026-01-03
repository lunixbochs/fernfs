use fernfs::xdr::nfs3::{createverf3, nfspath3, sattr3};

/// Enumeration for the create_fs_object method
pub enum CreateFSObject {
    /// Creates a directory
    Directory,
    /// Creates a file with a set of attributes
    File(sattr3),
    /// Creates an exclusive file with a verifier
    Exclusive(createverf3),
    /// Creates a symlink with a set of attributes to a target location
    Symlink((sattr3, nfspath3)),
}

impl CreateFSObject {
    /// Checks if the object is a directory
    pub fn is_directory(&self) -> bool {
        matches!(self, CreateFSObject::Directory)
    }

    /// Checks if the object is a file
    pub fn is_file(&self) -> bool {
        matches!(self, CreateFSObject::File(_))
    }

    /// Checks if the object is an exclusive file
    pub fn is_exclusive(&self) -> bool {
        matches!(self, CreateFSObject::Exclusive(_))
    }

    /// Checks if the object is a symlink
    pub fn is_symlink(&self) -> bool {
        matches!(self, CreateFSObject::Symlink(_))
    }

    /// Gets the attributes of the object if available
    pub fn get_attributes(&self) -> Option<&sattr3> {
        match self {
            CreateFSObject::File(attrs) => Some(attrs),
            CreateFSObject::Symlink((attrs, _)) => Some(attrs),
            _ => None,
        }
    }

    /// Gets the symlink target if the object is a symlink
    pub fn get_symlink_target(&self) -> Option<&nfspath3> {
        match self {
            CreateFSObject::Symlink((_, target)) => Some(target),
            _ => None,
        }
    }
}
