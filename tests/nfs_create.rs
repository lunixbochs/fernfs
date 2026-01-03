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
const CREATED_ID: nfs3::fileid3 = 2;

#[derive(Default)]
struct ExclusiveCaptureFS {
    captured: Mutex<Option<nfs3::createverf3>>,
    generation: u64,
}

impl ExclusiveCaptureFS {
    fn new() -> Self {
        Self { captured: Mutex::new(None), generation: 42 }
    }
}

#[async_trait]
impl vfs::NFSFileSystem for ExclusiveCaptureFS {
    fn generation(&self) -> u64 {
        self.generation
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::ReadWrite
    }

    fn root_dir(&self) -> nfs3::fileid3 {
        ROOT_ID
    }

    async fn lookup(
        &self,
        _dirid: nfs3::fileid3,
        _filename: &nfs3::filename3,
    ) -> Result<nfs3::fileid3, nfs3::nfsstat3> {
        Err(nfs3::nfsstat3::NFS3ERR_NOTSUPP)
    }

    async fn getattr(&self, id: nfs3::fileid3) -> Result<nfs3::fattr3, nfs3::nfsstat3> {
        if id != ROOT_ID {
            return Err(nfs3::nfsstat3::NFS3ERR_NOENT);
        }
        Ok(nfs3::fattr3 { ftype: nfs3::ftype3::NF3DIR, fileid: id, ..Default::default() })
    }

    async fn setattr(
        &self,
        _id: nfs3::fileid3,
        _setattr: nfs3::sattr3,
    ) -> Result<nfs3::fattr3, nfs3::nfsstat3> {
        Err(nfs3::nfsstat3::NFS3ERR_NOTSUPP)
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
        Err(nfs3::nfsstat3::NFS3ERR_NOTSUPP)
    }

    async fn create(
        &self,
        _dirid: nfs3::fileid3,
        _filename: &nfs3::filename3,
        _attr: nfs3::sattr3,
    ) -> Result<(nfs3::fileid3, nfs3::fattr3), nfs3::nfsstat3> {
        Err(nfs3::nfsstat3::NFS3ERR_NOTSUPP)
    }

    async fn create_exclusive(
        &self,
        _dirid: nfs3::fileid3,
        _filename: &nfs3::filename3,
        verifier: nfs3::createverf3,
    ) -> Result<nfs3::fileid3, nfs3::nfsstat3> {
        *self.captured.lock().unwrap() = Some(verifier);
        Ok(CREATED_ID)
    }

    async fn mkdir(
        &self,
        _dirid: nfs3::fileid3,
        _dirname: &nfs3::filename3,
    ) -> Result<(nfs3::fileid3, nfs3::fattr3), nfs3::nfsstat3> {
        Err(nfs3::nfsstat3::NFS3ERR_NOTSUPP)
    }

    async fn remove(
        &self,
        _dirid: nfs3::fileid3,
        _filename: &nfs3::filename3,
    ) -> Result<(), nfs3::nfsstat3> {
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
        _attrs: &nfs3::sattr3,
    ) -> Result<(nfs3::fileid3, nfs3::fattr3), nfs3::nfsstat3> {
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
}

#[tokio::test]
async fn create_exclusive_passes_verifier_to_vfs() {
    let fs = Arc::new(ExclusiveCaptureFS::new());
    let context = Context {
        local_port: 0,
        client_addr: "127.0.0.1:1234".to_string(),
        auth: xdr::rpc::auth_unix::default(),
        vfs: fs.clone(),
        mount_signal: None,
        export_name: Arc::from("/".to_string()),
        transaction_tracker: Arc::new(TransactionTracker::new(Duration::from_secs(60))),
        portmap_table: Arc::new(RwLock::new(PortmapTable::default())),
    };

    let dir_handle = fs.id_to_fh(ROOT_ID);
    let dirops = nfs3::diropargs3 { dir: dir_handle, name: b"file".as_slice().into() };
    let verifier: nfs3::createverf3 = [1, 2, 3, 4, 5, 6, 7, 8];

    let mut input = Cursor::new(Vec::new());
    dirops.serialize(&mut input).expect("serialize diropargs");
    nfs3::createmode3::EXCLUSIVE.serialize(&mut input).expect("serialize create mode");
    verifier.serialize(&mut input).expect("serialize verifier");
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
    handle_nfs(9, call, &mut input, &mut output, &context).await.expect("handle_nfs");

    assert_eq!(*fs.captured.lock().unwrap(), Some(verifier));

    output.set_position(0);
    let _rpc = xdr::deserialize::<xdr::rpc::rpc_msg>(&mut output).expect("deserialize rpc");
    let status_raw = xdr::deserialize::<u32>(&mut output).expect("deserialize status");
    let status = nfs3::nfsstat3::from_u32(status_raw).expect("invalid nfsstat3 value");
    assert_eq!(status, nfs3::nfsstat3::NFS3_OK);
    let fh = xdr::deserialize::<nfs3::post_op_fh3>(&mut output).expect("deserialize fh");
    let fh = fh.expect("expected file handle");
    assert_eq!(fs.fh_to_id(&fh).expect("fh_to_id"), CREATED_ID);
    let _ = xdr::deserialize::<nfs3::post_op_attr>(&mut output).expect("deserialize attr");
    let _ = xdr::deserialize::<nfs3::wcc_data>(&mut output).expect("deserialize wcc");
}
