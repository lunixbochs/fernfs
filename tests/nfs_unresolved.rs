use std::collections::HashMap;
use std::io::Cursor;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use async_trait::async_trait;
use num_traits::FromPrimitive;

use fernfs::protocol::nfs::portmap::PortmapTable;
use fernfs::protocol::nfs::v3::handle_nfs;
use fernfs::protocol::rpc::{Context, TransactionTracker};
use fernfs::vfs::{self, Capabilities, NFSFileSystem, ReadDirResult};
use fernfs::xdr::{self, nfs3, Serialize};

const ROOT_ID: nfs3::fileid3 = 1;

struct TestFS {
    generation: u64,
    capabilities: Capabilities,
    root_id: nfs3::fileid3,
    attrs: Mutex<HashMap<nfs3::fileid3, nfs3::fattr3>>,
    lookup_ids: Mutex<HashMap<(nfs3::fileid3, Vec<u8>), nfs3::fileid3>>,
    remove_result: Mutex<Option<Result<(), nfs3::nfsstat3>>>,
    mkdir_result: Mutex<Option<Result<(nfs3::fileid3, nfs3::fattr3), nfs3::nfsstat3>>>,
    create_result: Mutex<Option<Result<(nfs3::fileid3, nfs3::fattr3), nfs3::nfsstat3>>>,
    mknod_result: Mutex<Option<Result<(nfs3::fileid3, nfs3::fattr3), nfs3::nfsstat3>>>,
    mknod_attrs: Mutex<Option<nfs3::sattr3>>,
    write_result: Mutex<
        Option<Result<(nfs3::fattr3, nfs3::file::stable_how, nfs3::count3), nfs3::nfsstat3>>,
    >,
    readdir_result: Mutex<Option<Result<ReadDirResult, nfs3::nfsstat3>>>,
    fsinfo_result: Mutex<Option<Result<nfs3::fs::fsinfo3, nfs3::nfsstat3>>>,
    setattr_calls: Mutex<Vec<nfs3::sattr3>>,
}

impl TestFS {
    fn new() -> Self {
        Self {
            generation: 1,
            capabilities: Capabilities::ReadWrite,
            root_id: ROOT_ID,
            attrs: Mutex::new(HashMap::new()),
            lookup_ids: Mutex::new(HashMap::new()),
            remove_result: Mutex::new(None),
            mkdir_result: Mutex::new(None),
            create_result: Mutex::new(None),
            mknod_result: Mutex::new(None),
            mknod_attrs: Mutex::new(None),
            write_result: Mutex::new(None),
            readdir_result: Mutex::new(None),
            fsinfo_result: Mutex::new(None),
            setattr_calls: Mutex::new(Vec::new()),
        }
    }

    fn insert_attr(&self, id: nfs3::fileid3, attr: nfs3::fattr3) {
        self.attrs.lock().unwrap().insert(id, attr);
    }

    fn insert_lookup(&self, dirid: nfs3::fileid3, name: &[u8], id: nfs3::fileid3) {
        self.lookup_ids.lock().unwrap().insert((dirid, name.to_vec()), id);
    }
}

#[async_trait]
impl vfs::NFSFileSystem for TestFS {
    fn generation(&self) -> u64 {
        self.generation
    }

    fn capabilities(&self) -> Capabilities {
        match self.capabilities {
            Capabilities::ReadOnly => Capabilities::ReadOnly,
            Capabilities::ReadWrite => Capabilities::ReadWrite,
        }
    }

    fn root_dir(&self) -> nfs3::fileid3 {
        self.root_id
    }

    async fn lookup(
        &self,
        dirid: nfs3::fileid3,
        filename: &nfs3::filename3,
    ) -> Result<nfs3::fileid3, nfs3::nfsstat3> {
        self.lookup_ids
            .lock()
            .unwrap()
            .get(&(dirid, filename.as_ref().to_vec()))
            .copied()
            .ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)
    }

    async fn getattr(&self, id: nfs3::fileid3) -> Result<nfs3::fattr3, nfs3::nfsstat3> {
        self.attrs
            .lock()
            .unwrap()
            .get(&id)
            .copied()
            .ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)
    }

    async fn setattr(
        &self,
        id: nfs3::fileid3,
        setattr: nfs3::sattr3,
    ) -> Result<nfs3::fattr3, nfs3::nfsstat3> {
        self.setattr_calls.lock().unwrap().push(setattr);
        let mut attrs = self.attrs.lock().unwrap();
        let entry = attrs.get_mut(&id).ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)?;
        if let Some(mode) = setattr.mode {
            entry.mode = mode;
        }
        if let Some(uid) = setattr.uid {
            entry.uid = uid;
        }
        if let Some(gid) = setattr.gid {
            entry.gid = gid;
        }
        if let Some(size) = setattr.size {
            entry.size = size;
        }
        match setattr.atime {
            nfs3::set_atime::SET_TO_CLIENT_TIME(t) => entry.atime = t,
            nfs3::set_atime::SET_TO_SERVER_TIME => entry.atime = nfs3::nfstime3::default(),
            nfs3::set_atime::DONT_CHANGE => {}
        }
        match setattr.mtime {
            nfs3::set_mtime::SET_TO_CLIENT_TIME(t) => entry.mtime = t,
            nfs3::set_mtime::SET_TO_SERVER_TIME => entry.mtime = nfs3::nfstime3::default(),
            nfs3::set_mtime::DONT_CHANGE => {}
        }
        Ok(*entry)
    }

    async fn read(
        &self,
        _id: nfs3::fileid3,
        _offset: u64,
        _count: u32,
    ) -> Result<(Vec<u8>, bool), nfs3::nfsstat3> {
        Err(nfs3::nfsstat3::NFS3ERR_NOTSUPP)
    }

    async fn write(
        &self,
        _id: nfs3::fileid3,
        _offset: u64,
        _data: &[u8],
        _stable: nfs3::file::stable_how,
    ) -> Result<(nfs3::fattr3, nfs3::file::stable_how, nfs3::count3), nfs3::nfsstat3> {
        if let Some(result) = self.write_result.lock().unwrap().take() {
            return result;
        }
        Err(nfs3::nfsstat3::NFS3ERR_NOTSUPP)
    }

    async fn create(
        &self,
        _dirid: nfs3::fileid3,
        _filename: &nfs3::filename3,
        _attr: nfs3::sattr3,
    ) -> Result<(nfs3::fileid3, nfs3::fattr3), nfs3::nfsstat3> {
        if let Some(result) = self.create_result.lock().unwrap().take() {
            return result;
        }
        Err(nfs3::nfsstat3::NFS3ERR_NOTSUPP)
    }

    async fn create_exclusive(
        &self,
        _dirid: nfs3::fileid3,
        _filename: &nfs3::filename3,
        _verifier: nfs3::createverf3,
    ) -> Result<nfs3::fileid3, nfs3::nfsstat3> {
        Err(nfs3::nfsstat3::NFS3ERR_NOTSUPP)
    }

    async fn mkdir(
        &self,
        _dirid: nfs3::fileid3,
        _dirname: &nfs3::filename3,
    ) -> Result<(nfs3::fileid3, nfs3::fattr3), nfs3::nfsstat3> {
        if let Some(result) = self.mkdir_result.lock().unwrap().take() {
            return result;
        }
        Err(nfs3::nfsstat3::NFS3ERR_NOTSUPP)
    }

    async fn remove(
        &self,
        _dirid: nfs3::fileid3,
        _filename: &nfs3::filename3,
    ) -> Result<(), nfs3::nfsstat3> {
        if let Some(result) = self.remove_result.lock().unwrap().take() {
            return result;
        }
        Err(nfs3::nfsstat3::NFS3ERR_NOTSUPP)
    }

    async fn rename(
        &self,
        _from_dirid: nfs3::fileid3,
        _from_filename: &nfs3::filename3,
        _to_dirid: nfs3::fileid3,
        _to_filename: &nfs3::filename3,
    ) -> Result<(), nfs3::nfsstat3> {
        Err(nfs3::nfsstat3::NFS3ERR_NOTSUPP)
    }

    async fn readdir(
        &self,
        _dirid: nfs3::fileid3,
        _start_after: nfs3::fileid3,
        _max_entries: usize,
    ) -> Result<ReadDirResult, nfs3::nfsstat3> {
        if let Some(result) = self.readdir_result.lock().unwrap().take() {
            return result;
        }
        Err(nfs3::nfsstat3::NFS3ERR_NOTSUPP)
    }

    async fn symlink(
        &self,
        _dirid: nfs3::fileid3,
        _linkname: &nfs3::filename3,
        _symlink: &nfs3::nfspath3,
        _attr: &nfs3::sattr3,
    ) -> Result<(nfs3::fileid3, nfs3::fattr3), nfs3::nfsstat3> {
        Err(nfs3::nfsstat3::NFS3ERR_NOTSUPP)
    }

    async fn readlink(&self, _id: nfs3::fileid3) -> Result<nfs3::nfspath3, nfs3::nfsstat3> {
        Err(nfs3::nfsstat3::NFS3ERR_NOTSUPP)
    }

    async fn link(
        &self,
        _file_id: nfs3::fileid3,
        _link_dir_id: nfs3::fileid3,
        _link_name: &nfs3::filename3,
    ) -> Result<nfs3::fattr3, nfs3::nfsstat3> {
        Err(nfs3::nfsstat3::NFS3ERR_NOTSUPP)
    }

    async fn mknod(
        &self,
        _dir_id: nfs3::fileid3,
        _name: &nfs3::filename3,
        _ftype: nfs3::ftype3,
        _specdata: nfs3::specdata3,
        attrs: &nfs3::sattr3,
    ) -> Result<(nfs3::fileid3, nfs3::fattr3), nfs3::nfsstat3> {
        *self.mknod_attrs.lock().unwrap() = Some(*attrs);
        if let Some(result) = self.mknod_result.lock().unwrap().take() {
            return result;
        }
        Err(nfs3::nfsstat3::NFS3ERR_NOTSUPP)
    }

    async fn commit(
        &self,
        _file_id: nfs3::fileid3,
        _offset: u64,
        _count: u32,
    ) -> Result<nfs3::fattr3, nfs3::nfsstat3> {
        Err(nfs3::nfsstat3::NFS3ERR_NOTSUPP)
    }

    async fn fsinfo(
        &self,
        _root_fileid: nfs3::fileid3,
    ) -> Result<nfs3::fs::fsinfo3, nfs3::nfsstat3> {
        if let Some(result) = self.fsinfo_result.lock().unwrap().take() {
            return result;
        }
        Err(nfs3::nfsstat3::NFS3ERR_NOTSUPP)
    }
}

fn make_context(fs: Arc<TestFS>) -> Context {
    Context {
        local_port: 0,
        client_addr: "127.0.0.1:1234".to_string(),
        auth: xdr::rpc::auth_unix::default(),
        vfs: fs,
        mount_signal: None,
        export_name: Arc::from("/".to_string()),
        transaction_tracker: Arc::new(TransactionTracker::new(Duration::from_secs(60))),
        portmap_table: Arc::new(RwLock::new(PortmapTable::default())),
    }
}

fn read_status(output: &mut Cursor<Vec<u8>>) -> nfs3::nfsstat3 {
    let _rpc = xdr::deserialize::<xdr::rpc::rpc_msg>(output).expect("deserialize rpc");
    let status_raw = xdr::deserialize::<u32>(output).expect("deserialize status");
    nfs3::nfsstat3::from_u32(status_raw).expect("invalid nfsstat3 value")
}

fn dir_attr(id: nfs3::fileid3) -> nfs3::fattr3 {
    nfs3::fattr3 { ftype: nfs3::ftype3::NF3DIR, fileid: id, ..Default::default() }
}

fn file_attr(id: nfs3::fileid3, mode: u32, size: u64) -> nfs3::fattr3 {
    nfs3::fattr3 { ftype: nfs3::ftype3::NF3REG, fileid: id, mode, size, ..Default::default() }
}

#[tokio::test]
async fn rmdir_rejects_non_directory_target() {
    let fs = Arc::new(TestFS {
        remove_result: Mutex::new(Some(Ok(()))),
        ..TestFS::new()
    });
    fs.insert_attr(ROOT_ID, dir_attr(ROOT_ID));
    fs.insert_attr(2, file_attr(2, 0, 0));
    fs.insert_lookup(ROOT_ID, b"file", 2);
    let context = make_context(fs.clone());

    let args = nfs3::diropargs3 {
        dir: fs.id_to_fh(ROOT_ID),
        name: b"file".as_ref().into(),
    };
    let mut input = Cursor::new(Vec::new());
    args.serialize(&mut input).expect("serialize rmdir args");
    input.set_position(0);

    let call = xdr::rpc::call_body {
        rpcvers: 2,
        prog: nfs3::PROGRAM,
        vers: nfs3::VERSION,
        proc: nfs3::NFSProgram::NFSPROC3_RMDIR as u32,
        cred: xdr::rpc::opaque_auth::default(),
        verf: xdr::rpc::opaque_auth::default(),
    };

    let mut output = Cursor::new(Vec::new());
    handle_nfs(1, call, &mut input, &mut output, &context)
        .await
        .expect("handle_nfs");

    output.set_position(0);
    let status = read_status(&mut output);
    assert_eq!(status, nfs3::nfsstat3::NFS3ERR_NOTDIR);
}

#[tokio::test]
async fn readlink_error_includes_post_op_attr() {
    let fs = Arc::new(TestFS::new());
    let context = make_context(fs);

    let handle = nfs3::nfs_fh3 { data: Vec::new() };
    let mut input = Cursor::new(Vec::new());
    handle.serialize(&mut input).expect("serialize readlink args");
    input.set_position(0);

    let call = xdr::rpc::call_body {
        rpcvers: 2,
        prog: nfs3::PROGRAM,
        vers: nfs3::VERSION,
        proc: nfs3::NFSProgram::NFSPROC3_READLINK as u32,
        cred: xdr::rpc::opaque_auth::default(),
        verf: xdr::rpc::opaque_auth::default(),
    };

    let mut output = Cursor::new(Vec::new());
    handle_nfs(2, call, &mut input, &mut output, &context)
        .await
        .expect("handle_nfs");

    output.set_position(0);
    let status = read_status(&mut output);
    assert_eq!(status, nfs3::nfsstat3::NFS3ERR_BADHANDLE);
    let _attr = xdr::deserialize::<nfs3::post_op_attr>(&mut output)
        .expect("deserialize post_op_attr");
}

#[tokio::test]
async fn setattr_error_includes_wcc_data() {
    let fs = Arc::new(TestFS::new());
    let context = make_context(fs);

    let args = nfs3::SETATTR3args {
        object: nfs3::nfs_fh3 { data: Vec::new() },
        new_attribute: nfs3::sattr3::default(),
        guard: None,
    };
    let mut input = Cursor::new(Vec::new());
    args.serialize(&mut input).expect("serialize setattr args");
    input.set_position(0);

    let call = xdr::rpc::call_body {
        rpcvers: 2,
        prog: nfs3::PROGRAM,
        vers: nfs3::VERSION,
        proc: nfs3::NFSProgram::NFSPROC3_SETATTR as u32,
        cred: xdr::rpc::opaque_auth::default(),
        verf: xdr::rpc::opaque_auth::default(),
    };

    let mut output = Cursor::new(Vec::new());
    handle_nfs(3, call, &mut input, &mut output, &context)
        .await
        .expect("handle_nfs");

    output.set_position(0);
    let status = read_status(&mut output);
    assert_eq!(status, nfs3::nfsstat3::NFS3ERR_BADHANDLE);
    let _wcc = xdr::deserialize::<nfs3::wcc_data>(&mut output)
        .expect("deserialize wcc_data");
}

#[tokio::test]
async fn rename_error_includes_two_wcc_data_blocks() {
    let fs = Arc::new(TestFS::new());
    let context = make_context(fs);

    let from = nfs3::diropargs3 {
        dir: nfs3::nfs_fh3 { data: Vec::new() },
        name: b"src".as_ref().into(),
    };
    let to = nfs3::diropargs3 {
        dir: nfs3::nfs_fh3 { data: Vec::new() },
        name: b"dst".as_ref().into(),
    };

    let mut input = Cursor::new(Vec::new());
    from.serialize(&mut input).expect("serialize from");
    to.serialize(&mut input).expect("serialize to");
    input.set_position(0);

    let call = xdr::rpc::call_body {
        rpcvers: 2,
        prog: nfs3::PROGRAM,
        vers: nfs3::VERSION,
        proc: nfs3::NFSProgram::NFSPROC3_RENAME as u32,
        cred: xdr::rpc::opaque_auth::default(),
        verf: xdr::rpc::opaque_auth::default(),
    };

    let mut output = Cursor::new(Vec::new());
    handle_nfs(4, call, &mut input, &mut output, &context)
        .await
        .expect("handle_nfs");

    output.set_position(0);
    let status = read_status(&mut output);
    assert_eq!(status, nfs3::nfsstat3::NFS3ERR_BADHANDLE);
    let _from_wcc = xdr::deserialize::<nfs3::wcc_data>(&mut output)
        .expect("deserialize from wcc_data");
    let _to_wcc = xdr::deserialize::<nfs3::wcc_data>(&mut output)
        .expect("deserialize to wcc_data");
}

#[tokio::test]
async fn fsinfo_error_includes_post_op_attr() {
    let fs = Arc::new(TestFS {
        fsinfo_result: Mutex::new(Some(Err(nfs3::nfsstat3::NFS3ERR_IO))),
        ..TestFS::new()
    });
    let context = make_context(fs.clone());

    let handle = fs.id_to_fh(ROOT_ID);
    let mut input = Cursor::new(Vec::new());
    handle.serialize(&mut input).expect("serialize fsinfo args");
    input.set_position(0);

    let call = xdr::rpc::call_body {
        rpcvers: 2,
        prog: nfs3::PROGRAM,
        vers: nfs3::VERSION,
        proc: nfs3::NFSProgram::NFSPROC3_FSINFO as u32,
        cred: xdr::rpc::opaque_auth::default(),
        verf: xdr::rpc::opaque_auth::default(),
    };

    let mut output = Cursor::new(Vec::new());
    handle_nfs(5, call, &mut input, &mut output, &context)
        .await
        .expect("handle_nfs");

    output.set_position(0);
    let status = read_status(&mut output);
    assert_eq!(status, nfs3::nfsstat3::NFS3ERR_IO);
    let _attr = xdr::deserialize::<nfs3::post_op_attr>(&mut output)
        .expect("deserialize post_op_attr");
}

#[tokio::test]
async fn readdir_rejects_bad_cookieverf() {
    let fs = Arc::new(TestFS {
        readdir_result: Mutex::new(Some(Ok(ReadDirResult { entries: Vec::new(), end: true }))),
        ..TestFS::new()
    });
    let mut attr = dir_attr(ROOT_ID);
    attr.mtime = nfs3::nfstime3 { seconds: 10, nseconds: 20 };
    fs.insert_attr(ROOT_ID, attr);
    let context = make_context(fs.clone());

    let args = nfs3::dir::READDIR3args {
        dir: fs.id_to_fh(ROOT_ID),
        cookie: 0,
        cookieverf: [1; nfs3::NFS3_COOKIEVERFSIZE as usize],
        dircount: 1024,
    };
    let mut input = Cursor::new(Vec::new());
    args.serialize(&mut input).expect("serialize readdir args");
    input.set_position(0);

    let call = xdr::rpc::call_body {
        rpcvers: 2,
        prog: nfs3::PROGRAM,
        vers: nfs3::VERSION,
        proc: nfs3::NFSProgram::NFSPROC3_READDIR as u32,
        cred: xdr::rpc::opaque_auth::default(),
        verf: xdr::rpc::opaque_auth::default(),
    };

    let mut output = Cursor::new(Vec::new());
    handle_nfs(6, call, &mut input, &mut output, &context)
        .await
        .expect("handle_nfs");

    output.set_position(0);
    let status = read_status(&mut output);
    assert_eq!(status, nfs3::nfsstat3::NFS3ERR_BAD_COOKIE);
}

#[tokio::test]
async fn readdirplus_rejects_bad_cookieverf() {
    let fs = Arc::new(TestFS {
        readdir_result: Mutex::new(Some(Ok(ReadDirResult { entries: Vec::new(), end: true }))),
        ..TestFS::new()
    });
    let mut attr = dir_attr(ROOT_ID);
    attr.mtime = nfs3::nfstime3 { seconds: 11, nseconds: 22 };
    fs.insert_attr(ROOT_ID, attr);
    let context = make_context(fs.clone());

    let args = nfs3::dir::READDIRPLUS3args {
        dir: fs.id_to_fh(ROOT_ID),
        cookie: 0,
        cookieverf: [2; nfs3::NFS3_COOKIEVERFSIZE as usize],
        dircount: 1024,
        maxcount: 2048,
    };
    let mut input = Cursor::new(Vec::new());
    args.serialize(&mut input).expect("serialize readdirplus args");
    input.set_position(0);

    let call = xdr::rpc::call_body {
        rpcvers: 2,
        prog: nfs3::PROGRAM,
        vers: nfs3::VERSION,
        proc: nfs3::NFSProgram::NFSPROC3_READDIRPLUS as u32,
        cred: xdr::rpc::opaque_auth::default(),
        verf: xdr::rpc::opaque_auth::default(),
    };

    let mut output = Cursor::new(Vec::new());
    handle_nfs(7, call, &mut input, &mut output, &context)
        .await
        .expect("handle_nfs");

    output.set_position(0);
    let status = read_status(&mut output);
    assert_eq!(status, nfs3::nfsstat3::NFS3ERR_BAD_COOKIE);
}

#[tokio::test]
async fn readdir_uses_sequential_cookies() {
    let fs = Arc::new(TestFS {
        readdir_result: Mutex::new(Some(Ok(ReadDirResult {
            entries: vec![
                vfs::DirEntry {
                    fileid: 100,
                    name: b"alpha".as_ref().into(),
                    attr: file_attr(100, 0, 0),
                },
                vfs::DirEntry {
                    fileid: 200,
                    name: b"beta".as_ref().into(),
                    attr: file_attr(200, 0, 0),
                },
            ],
            end: true,
        }))),
        ..TestFS::new()
    });
    fs.insert_attr(ROOT_ID, dir_attr(ROOT_ID));
    let context = make_context(fs.clone());

    let args = nfs3::dir::READDIR3args {
        dir: fs.id_to_fh(ROOT_ID),
        cookie: 0,
        cookieverf: nfs3::cookieverf3::default(),
        dircount: 4096,
    };
    let mut input = Cursor::new(Vec::new());
    args.serialize(&mut input).expect("serialize readdir args");
    input.set_position(0);

    let call = xdr::rpc::call_body {
        rpcvers: 2,
        prog: nfs3::PROGRAM,
        vers: nfs3::VERSION,
        proc: nfs3::NFSProgram::NFSPROC3_READDIR as u32,
        cred: xdr::rpc::opaque_auth::default(),
        verf: xdr::rpc::opaque_auth::default(),
    };

    let mut output = Cursor::new(Vec::new());
    handle_nfs(8, call, &mut input, &mut output, &context)
        .await
        .expect("handle_nfs");

    output.set_position(0);
    let status = read_status(&mut output);
    assert_eq!(status, nfs3::nfsstat3::NFS3_OK);
    let _attr = xdr::deserialize::<nfs3::post_op_attr>(&mut output)
        .expect("deserialize post_op_attr");
    let _verf = xdr::deserialize::<nfs3::cookieverf3>(&mut output)
        .expect("deserialize cookieverf");

    let mut cookies = Vec::new();
    loop {
        let has_entry = xdr::deserialize::<bool>(&mut output).expect("deserialize entry flag");
        if !has_entry {
            break;
        }
        let entry = xdr::deserialize::<nfs3::dir::entry3>(&mut output)
            .expect("deserialize entry3");
        cookies.push(entry.cookie);
    }
    let _eof = xdr::deserialize::<bool>(&mut output).expect("deserialize eof flag");
    assert_eq!(cookies, vec![1, 2]);
}

#[tokio::test]
async fn readdirplus_uses_sequential_cookies() {
    let fs = Arc::new(TestFS {
        readdir_result: Mutex::new(Some(Ok(ReadDirResult {
            entries: vec![
                vfs::DirEntry {
                    fileid: 300,
                    name: b"gamma".as_ref().into(),
                    attr: file_attr(300, 0, 0),
                },
                vfs::DirEntry {
                    fileid: 400,
                    name: b"delta".as_ref().into(),
                    attr: file_attr(400, 0, 0),
                },
            ],
            end: true,
        }))),
        ..TestFS::new()
    });
    fs.insert_attr(ROOT_ID, dir_attr(ROOT_ID));
    let context = make_context(fs.clone());

    let args = nfs3::dir::READDIRPLUS3args {
        dir: fs.id_to_fh(ROOT_ID),
        cookie: 0,
        cookieverf: nfs3::cookieverf3::default(),
        dircount: 4096,
        maxcount: 4096,
    };
    let mut input = Cursor::new(Vec::new());
    args.serialize(&mut input).expect("serialize readdirplus args");
    input.set_position(0);

    let call = xdr::rpc::call_body {
        rpcvers: 2,
        prog: nfs3::PROGRAM,
        vers: nfs3::VERSION,
        proc: nfs3::NFSProgram::NFSPROC3_READDIRPLUS as u32,
        cred: xdr::rpc::opaque_auth::default(),
        verf: xdr::rpc::opaque_auth::default(),
    };

    let mut output = Cursor::new(Vec::new());
    handle_nfs(9, call, &mut input, &mut output, &context)
        .await
        .expect("handle_nfs");

    output.set_position(0);
    let status = read_status(&mut output);
    assert_eq!(status, nfs3::nfsstat3::NFS3_OK);
    let _attr = xdr::deserialize::<nfs3::post_op_attr>(&mut output)
        .expect("deserialize post_op_attr");
    let _verf = xdr::deserialize::<nfs3::cookieverf3>(&mut output)
        .expect("deserialize cookieverf");

    let mut cookies = Vec::new();
    loop {
        let has_entry = xdr::deserialize::<bool>(&mut output).expect("deserialize entry flag");
        if !has_entry {
            break;
        }
        let entry = xdr::deserialize::<nfs3::dir::entryplus3>(&mut output)
            .expect("deserialize entryplus3");
        cookies.push(entry.cookie);
    }
    let _eof = xdr::deserialize::<bool>(&mut output).expect("deserialize eof flag");
    assert_eq!(cookies, vec![1, 2]);
}

#[tokio::test]
async fn mkdir_applies_requested_attributes() {
    let fs = Arc::new(TestFS {
        mkdir_result: Mutex::new(Some(Ok((2, dir_attr(2))))),
        ..TestFS::new()
    });
    fs.insert_attr(ROOT_ID, dir_attr(ROOT_ID));
    fs.insert_attr(2, dir_attr(2));
    let context = make_context(fs.clone());

    let mut attrs = nfs3::sattr3::default();
    attrs.mode = nfs3::set_mode3::Some(0o755);
    let args = nfs3::dir::MKDIR3args {
        dirops: nfs3::diropargs3 { dir: fs.id_to_fh(ROOT_ID), name: b"dir".as_ref().into() },
        attributes: attrs,
    };

    let mut input = Cursor::new(Vec::new());
    args.serialize(&mut input).expect("serialize mkdir args");
    input.set_position(0);

    let call = xdr::rpc::call_body {
        rpcvers: 2,
        prog: nfs3::PROGRAM,
        vers: nfs3::VERSION,
        proc: nfs3::NFSProgram::NFSPROC3_MKDIR as u32,
        cred: xdr::rpc::opaque_auth::default(),
        verf: xdr::rpc::opaque_auth::default(),
    };

    let mut output = Cursor::new(Vec::new());
    handle_nfs(7, call, &mut input, &mut output, &context)
        .await
        .expect("handle_nfs");

    output.set_position(0);
    let status = read_status(&mut output);
    assert_eq!(status, nfs3::nfsstat3::NFS3_OK);
    let _fh = xdr::deserialize::<nfs3::post_op_fh3>(&mut output).expect("deserialize fh");
    let attr = xdr::deserialize::<nfs3::post_op_attr>(&mut output)
        .expect("deserialize post_op_attr");
    match attr {
        nfs3::post_op_attr::Some(attr) => {
            assert_eq!(attr.mode, 0o755);
        }
        nfs3::post_op_attr::None => panic!("missing attributes"),
    }
}

#[tokio::test]
async fn create_unchecked_updates_existing_attributes() {
    let fs = Arc::new(TestFS {
        create_result: Mutex::new(Some(Ok((2, file_attr(2, 0, 0))))),
        ..TestFS::new()
    });
    fs.insert_attr(ROOT_ID, dir_attr(ROOT_ID));
    fs.insert_attr(2, file_attr(2, 0, 0));
    fs.insert_lookup(ROOT_ID, b"file", 2);
    let context = make_context(fs.clone());

    let mut attrs = nfs3::sattr3::default();
    attrs.mode = nfs3::set_mode3::Some(0o644);

    let dirops = nfs3::diropargs3 { dir: fs.id_to_fh(ROOT_ID), name: b"file".as_ref().into() };

    let mut input = Cursor::new(Vec::new());
    dirops.serialize(&mut input).expect("serialize dirops");
    nfs3::createmode3::UNCHECKED.serialize(&mut input).expect("serialize mode");
    attrs.serialize(&mut input).expect("serialize attrs");
    input.set_position(0);

    let call = xdr::rpc::call_body {
        rpcvers: 2,
        prog: nfs3::PROGRAM,
        vers: nfs3::VERSION,
        proc: nfs3::NFSProgram::NFSPROC3_CREATE as u32,
        cred: xdr::rpc::opaque_auth::default(),
        verf: xdr::rpc::opaque_auth::default(),
    };

    let mut output = Cursor::new(Vec::new());
    handle_nfs(8, call, &mut input, &mut output, &context)
        .await
        .expect("handle_nfs");

    output.set_position(0);
    let status = read_status(&mut output);
    assert_eq!(status, nfs3::nfsstat3::NFS3_OK);
    let _fh = xdr::deserialize::<nfs3::post_op_fh3>(&mut output).expect("deserialize fh");
    let attr = xdr::deserialize::<nfs3::post_op_attr>(&mut output)
        .expect("deserialize post_op_attr");
    match attr {
        nfs3::post_op_attr::Some(attr) => {
            assert_eq!(attr.mode, 0o644);
        }
        nfs3::post_op_attr::None => panic!("missing attributes"),
    }
}

#[tokio::test]
async fn mknod_passes_pipe_attributes() {
    let fs = Arc::new(TestFS {
        mknod_result: Mutex::new(Some(Ok((2, file_attr(2, 0, 0))))),
        ..TestFS::new()
    });
    fs.insert_attr(ROOT_ID, dir_attr(ROOT_ID));
    let context = make_context(fs.clone());

    let mut attrs = nfs3::sattr3::default();
    attrs.mode = nfs3::set_mode3::Some(0o600);

    let dirops = nfs3::diropargs3 { dir: fs.id_to_fh(ROOT_ID), name: b"pipe".as_ref().into() };

    let args = nfs3::dir::MKNOD3args {
        where_dir: dirops,
        what: nfs3::dir::mknoddata3 {
            mknod_type: nfs3::ftype3::NF3FIFO,
            device: None,
            pipe_attributes: Some(attrs),
        },
    };

    let mut input = Cursor::new(Vec::new());
    args.serialize(&mut input).expect("serialize mknod args");
    input.set_position(0);

    let call = xdr::rpc::call_body {
        rpcvers: 2,
        prog: nfs3::PROGRAM,
        vers: nfs3::VERSION,
        proc: nfs3::NFSProgram::NFSPROC3_MKNOD as u32,
        cred: xdr::rpc::opaque_auth::default(),
        verf: xdr::rpc::opaque_auth::default(),
    };

    let mut output = Cursor::new(Vec::new());
    handle_nfs(9, call, &mut input, &mut output, &context)
        .await
        .expect("handle_nfs");

    let captured = fs.mknod_attrs.lock().unwrap().expect("captured attrs");
    assert_eq!(captured.mode, nfs3::set_mode3::Some(0o600));
}

#[tokio::test]
async fn write_reports_actual_count() {
    let file_id = 2;
    let fs = Arc::new(TestFS {
        write_result: Mutex::new(Some(Ok((
            file_attr(file_id, 0, 2),
            nfs3::file::stable_how::FILE_SYNC,
            2,
        )))),
        ..TestFS::new()
    });
    fs.insert_attr(file_id, file_attr(file_id, 0, 2));
    let context = make_context(fs.clone());

    let args = nfs3::file::WRITE3args {
        file: fs.id_to_fh(file_id),
        offset: 0,
        count: 4,
        stable: nfs3::file::stable_how::FILE_SYNC as u32,
        data: b"data".to_vec(),
    };

    let mut input = Cursor::new(Vec::new());
    args.serialize(&mut input).expect("serialize write args");
    input.set_position(0);

    let call = xdr::rpc::call_body {
        rpcvers: 2,
        prog: nfs3::PROGRAM,
        vers: nfs3::VERSION,
        proc: nfs3::NFSProgram::NFSPROC3_WRITE as u32,
        cred: xdr::rpc::opaque_auth::default(),
        verf: xdr::rpc::opaque_auth::default(),
    };

    let mut output = Cursor::new(Vec::new());
    handle_nfs(10, call, &mut input, &mut output, &context)
        .await
        .expect("handle_nfs");

    output.set_position(0);
    let status = read_status(&mut output);
    assert_eq!(status, nfs3::nfsstat3::NFS3_OK);
    let res = xdr::deserialize::<nfs3::file::WRITE3resok>(&mut output)
        .expect("deserialize resok");
    assert_eq!(res.count, 2);
}

#[tokio::test]
async fn access_returns_only_requested_bits() {
    let file_id = 2;
    let fs = Arc::new(TestFS::new());
    fs.insert_attr(file_id, file_attr(file_id, 0, 0));
    let context = make_context(fs.clone());

    let handle = fs.id_to_fh(file_id);
    let mut input = Cursor::new(Vec::new());
    handle.serialize(&mut input).expect("serialize handle");
    nfs3::ACCESS3_READ.serialize(&mut input).expect("serialize access mask");
    input.set_position(0);

    let call = xdr::rpc::call_body {
        rpcvers: 2,
        prog: nfs3::PROGRAM,
        vers: nfs3::VERSION,
        proc: nfs3::NFSProgram::NFSPROC3_ACCESS as u32,
        cred: xdr::rpc::opaque_auth::default(),
        verf: xdr::rpc::opaque_auth::default(),
    };

    let mut output = Cursor::new(Vec::new());
    handle_nfs(11, call, &mut input, &mut output, &context)
        .await
        .expect("handle_nfs");

    output.set_position(0);
    let status = read_status(&mut output);
    assert_eq!(status, nfs3::nfsstat3::NFS3_OK);
    let _attr = xdr::deserialize::<nfs3::post_op_attr>(&mut output)
        .expect("deserialize post_op_attr");
    let granted = xdr::deserialize::<u32>(&mut output).expect("deserialize access");
    assert_eq!(granted, nfs3::ACCESS3_READ);
}
