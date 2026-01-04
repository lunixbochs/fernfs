use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use fernfs::vfs::NFSFileSystem;
use fernfs::xdr::nfs3;

#[allow(dead_code)]
#[path = "../src/bin/fernfs/create_fs_object.rs"]
mod create_fs_object;
#[allow(dead_code)]
#[path = "../src/bin/fernfs/error_handling.rs"]
mod error_handling;
#[allow(dead_code)]
#[path = "../src/bin/fernfs/fs_entry.rs"]
mod fs_entry;
#[allow(dead_code)]
#[path = "../src/bin/fernfs/fs_map.rs"]
mod fs_map;
#[allow(dead_code)]
#[path = "../src/bin/fernfs/fs.rs"]
mod mirror_fs;

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> std::io::Result<Self> {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        path.push(format!("fernfs_{prefix}_{nanos}"));
        std::fs::create_dir(&path)?;
        Ok(Self { path })
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[tokio::test]
async fn mirrorfs_exclusive_create_returns_not_supported() {
    let temp = TempDir::new("mirrorfs_exclusive").expect("temp dir");
    let fs = mirror_fs::MirrorFS::new(temp.path.clone());
    let name: nfs3::filename3 = b"exclusive_file".as_ref().into();
    let verifier: nfs3::createverf3 = [1, 2, 3, 4, 5, 6, 7, 8];

    let err = fs.create_exclusive(fs.root_dir(), &name, verifier).await.unwrap_err();
    assert_eq!(err, nfs3::nfsstat3::NFS3ERR_NOTSUPP);
}
