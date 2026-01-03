use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use nfs_mamont::vfs::NFSFileSystem;
use nfs_mamont::xdr::nfs3;

#[allow(dead_code)]
#[path = "../examples/mirror_fs/create_fs_object.rs"]
mod create_fs_object;
#[allow(dead_code)]
#[path = "../examples/mirror_fs/error_handling.rs"]
mod error_handling;
#[allow(dead_code)]
#[path = "../examples/mirror_fs/fs_entry.rs"]
mod fs_entry;
#[allow(dead_code)]
#[path = "../examples/mirror_fs/fs_map.rs"]
mod fs_map;
#[allow(dead_code)]
#[path = "../examples/mirror_fs/fs.rs"]
mod mirror_fs;

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> std::io::Result<Self> {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        path.push(format!("nfs_mamont_{prefix}_{nanos}"));
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
async fn readdir_paginates_with_duplicate_fileids() -> Result<(), nfs3::nfsstat3> {
    let temp = TempDir::new("mirrorfs_readdir").expect("temp dir");
    let fs = mirror_fs::MirrorFS::new(temp.path.clone());
    let root = fs.root_dir();

    for i in 0..5 {
        let name: nfs3::filename3 = format!("seed{i}").into_bytes().into();
        let _ = fs.create(root, &name, nfs3::sattr3::default()).await?;
    }

    let dir_name: nfs3::filename3 = b"dir".as_ref().into();
    let (dir_id, _) = fs.mkdir(root, &dir_name).await?;
    let file_name: nfs3::filename3 = b"file".as_ref().into();
    let (file_id, _) = fs.create(dir_id, &file_name, nfs3::sattr3::default()).await?;
    let file2_name: nfs3::filename3 = b"file2".as_ref().into();
    let _ = fs.link(file_id, dir_id, &file2_name).await?;

    let mut names = Vec::new();
    let mut cookie = 0u64;
    for _ in 0..10 {
        let result = fs.readdir(dir_id, cookie, 1).await?;
        if result.entries.is_empty() {
            break;
        }
        let name = String::from_utf8_lossy(&result.entries[0].name).to_string();
        names.push(name);
        cookie = cookie.saturating_add(result.entries.len() as u64);
        if result.end {
            break;
        }
    }

    assert_eq!(names, vec![".", "..", "file", "file2"]);
    Ok(())
}

#[tokio::test]
async fn rename_directory_updates_descendant_aliases() -> Result<(), nfs3::nfsstat3> {
    let temp = TempDir::new("mirrorfs_rename").expect("temp dir");
    let fs = mirror_fs::MirrorFS::new(temp.path.clone());
    let root = fs.root_dir();

    let old_name: nfs3::filename3 = b"old".as_ref().into();
    let (old_dir, _) = fs.mkdir(root, &old_name).await?;
    let child_name: nfs3::filename3 = b"child".as_ref().into();
    let (file_id, _) = fs.create(old_dir, &child_name, nfs3::sattr3::default()).await?;

    let new_name: nfs3::filename3 = b"new".as_ref().into();
    fs.rename(root, &old_name, root, &new_name).await?;

    let new_dir = fs.lookup(root, &new_name).await?;
    let new_file = fs.lookup(new_dir, &child_name).await?;
    assert_eq!(new_file, file_id);

    let attr = fs.getattr(file_id).await?;
    assert_eq!(attr.fileid, file_id);

    assert_eq!(
        fs.lookup(root, &old_name).await.unwrap_err(),
        nfs3::nfsstat3::NFS3ERR_NOENT
    );

    Ok(())
}
