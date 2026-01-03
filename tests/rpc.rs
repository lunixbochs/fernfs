use std::io::Cursor;
use std::sync::{Arc, RwLock};
use std::time::Duration;

mod support;

use tokio::io::AsyncWriteExt;
use tokio::time::timeout;

use nfs_mamont::protocol::nfs::portmap::PortmapTable;
use nfs_mamont::protocol::rpc::{Context, SocketMessageHandler, TransactionTracker};
use nfs_mamont::xdr::{self, nfs3, Serialize};

use support::DemoFS;

fn test_context() -> Context {
    Context {
        local_port: 0,
        client_addr: "127.0.0.1:1234".to_string(),
        auth: xdr::rpc::auth_unix::default(),
        vfs: Arc::new(DemoFS::default()),
        mount_signal: None,
        export_name: Arc::from("/".to_string()),
        transaction_tracker: Arc::new(TransactionTracker::new(Duration::from_secs(60))),
        portmap_table: Arc::new(RwLock::new(PortmapTable::default())),
    }
}

#[tokio::test]
async fn rejects_oversized_rpc_fragment() {
    let (mut handler, mut socksend, _msgrecv) = SocketMessageHandler::new(&test_context());

    let oversized = nfs_mamont::protocol::rpc::MAX_RPC_RECORD_LENGTH + 1;
    let fragment_header = (1_u32 << 31) | (oversized as u32);
    socksend
        .write_all(&fragment_header.to_be_bytes())
        .await
        .expect("write fragment header");

    let err = handler.read().await.expect_err("expected oversize error");
    assert!(
        err.to_string().contains("exceeds max"),
        "unexpected error: {err:?}"
    );
}

#[tokio::test]
async fn accepts_rpc_fragment_under_limit() {
    let xid = 7;
    let call = xdr::rpc::call_body {
        rpcvers: 2,
        prog: nfs3::PROGRAM,
        vers: nfs3::VERSION + 1,
        proc: 0,
        cred: xdr::rpc::opaque_auth::default(),
        verf: xdr::rpc::opaque_auth::default(),
    };
    let msg = xdr::rpc::rpc_msg { xid, body: xdr::rpc::rpc_body::CALL(call) };
    let mut msg_buf = Vec::new();
    msg.serialize(&mut msg_buf).expect("serialize rpc_msg");

    let (mut handler, mut socksend, mut msgrecv) = SocketMessageHandler::new(&test_context());
    let fragment_header = (1_u32 << 31) | (msg_buf.len() as u32);
    socksend
        .write_all(&fragment_header.to_be_bytes())
        .await
        .expect("write fragment header");
    socksend.write_all(&msg_buf).await.expect("write fragment body");

    handler.read().await.expect("handler read");

    let response = timeout(Duration::from_secs(1), msgrecv.recv())
        .await
        .expect("response timeout")
        .expect("response channel closed")
        .expect("response error");
    let reply = xdr::deserialize::<xdr::rpc::rpc_msg>(&mut Cursor::new(response))
        .expect("deserialize reply");
    assert_eq!(reply.xid, xid);
}

#[tokio::test]
async fn returns_prog_mismatch_for_unsupported_nfs_version() {
    let xid = 42;
    let call = xdr::rpc::call_body {
        rpcvers: 2,
        prog: nfs3::PROGRAM,
        vers: nfs3::VERSION + 1,
        proc: 0,
        cred: xdr::rpc::opaque_auth::default(),
        verf: xdr::rpc::opaque_auth::default(),
    };
    let msg = xdr::rpc::rpc_msg { xid, body: xdr::rpc::rpc_body::CALL(call) };
    let mut msg_buf = Vec::new();
    msg.serialize(&mut msg_buf).expect("serialize rpc_msg");

    let (mut handler, mut socksend, mut msgrecv) = SocketMessageHandler::new(&test_context());

    let fragment_header = (1_u32 << 31) | (msg_buf.len() as u32);
    socksend
        .write_all(&fragment_header.to_be_bytes())
        .await
        .expect("write fragment header");
    socksend.write_all(&msg_buf).await.expect("write fragment body");

    handler.read().await.expect("handler read");

    let response = timeout(Duration::from_secs(1), msgrecv.recv())
        .await
        .expect("response timeout")
        .expect("response channel closed")
        .expect("response error");

    let reply = xdr::deserialize::<xdr::rpc::rpc_msg>(&mut Cursor::new(response))
        .expect("deserialize reply");
    assert_eq!(reply.xid, xid);
    match reply.body {
        xdr::rpc::rpc_body::REPLY(xdr::rpc::reply_body::MSG_ACCEPTED(accepted)) => {
            match accepted.reply_data {
                xdr::rpc::accept_body::PROG_MISMATCH(info) => {
                    assert_eq!(info.low, nfs3::VERSION);
                    assert_eq!(info.high, nfs3::VERSION);
                }
                other => panic!("expected PROG_MISMATCH, got {:?}", other),
            }
        }
        other => panic!("expected MSG_ACCEPTED, got {:?}", other),
    }
}

#[tokio::test]
async fn retransmission_replays_response() {
    let xid = 99;
    let call = xdr::rpc::call_body {
        rpcvers: 2,
        prog: nfs3::PROGRAM,
        vers: nfs3::VERSION,
        proc: nfs3::NFSProgram::NFSPROC3_NULL as u32,
        cred: xdr::rpc::opaque_auth::default(),
        verf: xdr::rpc::opaque_auth::default(),
    };
    let msg = xdr::rpc::rpc_msg { xid, body: xdr::rpc::rpc_body::CALL(call) };
    let mut msg_buf = Vec::new();
    msg.serialize(&mut msg_buf).expect("serialize rpc_msg");

    let (mut handler, mut socksend, mut msgrecv) = SocketMessageHandler::new(&test_context());
    let fragment_header = (1_u32 << 31) | (msg_buf.len() as u32);

    socksend
        .write_all(&fragment_header.to_be_bytes())
        .await
        .expect("write fragment header");
    socksend.write_all(&msg_buf).await.expect("write fragment body");
    handler.read().await.expect("handler read");

    let response = timeout(Duration::from_secs(1), msgrecv.recv())
        .await
        .expect("response timeout")
        .expect("response channel closed")
        .expect("response error");
    let reply = xdr::deserialize::<xdr::rpc::rpc_msg>(&mut Cursor::new(response))
        .expect("deserialize reply");
    assert_eq!(reply.xid, xid);

    socksend
        .write_all(&fragment_header.to_be_bytes())
        .await
        .expect("write fragment header");
    socksend.write_all(&msg_buf).await.expect("write fragment body");
    handler.read().await.expect("handler read");

    let response = timeout(Duration::from_secs(1), msgrecv.recv())
        .await
        .expect("response timeout")
        .expect("response channel closed")
        .expect("response error");
    let reply = xdr::deserialize::<xdr::rpc::rpc_msg>(&mut Cursor::new(response))
        .expect("deserialize reply");
    assert_eq!(reply.xid, xid);
}
