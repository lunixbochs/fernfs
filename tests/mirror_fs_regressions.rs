use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use fernfs::vfs::NFSFileSystem;
use fernfs::xdr::nfs3;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

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

    assert_eq!(fs.lookup(root, &old_name).await.unwrap_err(), nfs3::nfsstat3::NFS3ERR_NOENT);

    Ok(())
}

#[tokio::test]
async fn refresh_dir_list_does_not_skip_when_metadata_is_stale() -> Result<(), nfs3::nfsstat3> {
    let temp = TempDir::new("mirrorfs_refresh_dir_list").expect("temp dir");
    std::fs::write(temp.path.join("a"), b"seed").expect("write seed");

    let mut fsmap = fs_map::FSMap::new(temp.path.clone());
    fsmap.refresh_dir_list(0).await?;

    let original_meta = fsmap.id_to_path.get(&0).expect("root entry").fsmeta;

    std::fs::write(temp.path.join("b"), b"next").expect("write next");
    if let Some(entry_mut) = fsmap.id_to_path.get_mut(&0) {
        // Simulate a backend where directory metadata does not reflect entry changes.
        entry_mut.fsmeta = original_meta;
        entry_mut.children_meta = original_meta;
    }

    fsmap.refresh_dir_list(0).await?;
    let children = fsmap
        .id_to_path
        .get(&0)
        .expect("root entry")
        .children
        .as_ref()
        .expect("children populated");
    let names: Vec<String> = children
        .keys()
        .map(|sym| fsmap.intern.get(*sym).unwrap().to_string_lossy().into_owned())
        .collect();
    assert!(
        names.contains(&"b".to_string()),
        "expected refresh to include new file, got {names:?}"
    );

    Ok(())
}

#[tokio::test]
async fn lookup_returns_noent_after_out_of_band_delete() -> Result<(), nfs3::nfsstat3> {
    let temp = TempDir::new("mirrorfs_lookup_stale").expect("temp dir");
    let fs = mirror_fs::MirrorFS::new(temp.path.clone());
    let root = fs.root_dir();

    let name: nfs3::filename3 = b"stale.txt".as_ref().into();
    let _ = fs.create(root, &name, nfs3::sattr3::default()).await?;
    let _ = fs.lookup(root, &name).await?;

    std::fs::remove_file(temp.path.join("stale.txt")).expect("remove stale file");

    let err = fs.lookup(root, &name).await.unwrap_err();
    assert_eq!(err, nfs3::nfsstat3::NFS3ERR_NOENT);

    Ok(())
}

#[tokio::test]
async fn lookup_succeeds_after_atomic_replace() -> Result<(), nfs3::nfsstat3> {
    let temp = TempDir::new("mirrorfs_lookup_replace").expect("temp dir");
    let fs = mirror_fs::MirrorFS::new(temp.path.clone());
    let root = fs.root_dir();

    let name: nfs3::filename3 = b"swap.txt".as_ref().into();
    let (old_id, _) = fs.create(root, &name, nfs3::sattr3::default()).await?;
    let _ = fs.lookup(root, &name).await?;

    let replacement = temp.path.join("swap.txt.new");
    std::fs::write(&replacement, b"new").expect("write replacement");
    std::fs::rename(&replacement, temp.path.join("swap.txt")).expect("rename replacement");

    let new_id = fs.lookup(root, &name).await?;
    assert_ne!(new_id, old_id);

    let attr = fs.getattr(new_id).await?;
    assert_eq!(attr.size, 3);

    Ok(())
}

#[tokio::test]
#[cfg(unix)]
async fn readdir_errors_when_directory_is_unreadable() -> Result<(), nfs3::nfsstat3> {
    let temp = TempDir::new("mirrorfs_readdir_perm").expect("temp dir");
    let fs = mirror_fs::MirrorFS::new(temp.path.clone());
    let root = fs.root_dir();

    let dir_name: nfs3::filename3 = b"private".as_ref().into();
    let (dir_id, _) = fs.mkdir(root, &dir_name).await?;
    let file_name: nfs3::filename3 = b"file".as_ref().into();
    let _ = fs.create(dir_id, &file_name, nfs3::sattr3::default()).await?;

    let _ = fs.readdir(dir_id, 0, 10).await?;

    let dir_path = temp.path.join("private");
    let original_perm = std::fs::metadata(&dir_path).expect("metadata").permissions();
    std::fs::set_permissions(&dir_path, std::fs::Permissions::from_mode(0o000))
        .expect("chmod private dir");

    let result = fs.readdir(dir_id, 0, 10).await;

    std::fs::set_permissions(&dir_path, original_perm).expect("restore permissions");

    let err = result.unwrap_err();
    assert!(
        matches!(err, nfs3::nfsstat3::NFS3ERR_ACCES | nfs3::nfsstat3::NFS3ERR_IO),
        "expected access error, got {err:?}"
    );

    Ok(())
}

#[tokio::test]
async fn readdir_reflects_file_size_after_write() -> Result<(), nfs3::nfsstat3> {
    let temp = TempDir::new("mirrorfs_readdir_size").expect("temp dir");
    let fs = mirror_fs::MirrorFS::new(temp.path.clone());
    let root = fs.root_dir();

    let name: nfs3::filename3 = b"data.bin".as_ref().into();
    let (file_id, _) = fs.create(root, &name, nfs3::sattr3::default()).await?;

    let _ = fs.readdir(root, 0, 10).await?;

    let data = b"hello world";
    let _ = fs.write(file_id, 0, data, nfs3::file::stable_how::FILE_SYNC).await?;

    let listing = fs.readdir(root, 0, 10).await?;
    let entry = listing
        .entries
        .into_iter()
        .find(|entry| entry.name.as_ref() == b"data.bin")
        .expect("missing data.bin entry");

    assert_eq!(entry.attr.size, data.len() as u64);

    Ok(())
}
