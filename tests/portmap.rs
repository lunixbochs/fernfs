use std::io::Cursor;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use async_trait::async_trait;
use num_traits::ToPrimitive;

use nfs_mamont::protocol::nfs::portmap::PortmapTable;
use nfs_mamont::protocol::rpc;
use nfs_mamont::protocol::rpc::Context;
use nfs_mamont::vfs::{Capabilities, ReadDirResult};
use nfs_mamont::xdr::nfs3::{
    fattr3, fileid3, filename3, ftype3, nfspath3, nfsstat3, sattr3, specdata3,
};
use nfs_mamont::xdr::portmap::{mapping, IPPROTO_TCP, IPPROTO_UDP};
use nfs_mamont::xdr::rpc::call_body;
use nfs_mamont::xdr::{deserialize, nfs3, Serialize};
use nfs_mamont::{vfs, xdr};

pub struct DemoFS {
    _root: String,
}

const RPC_MSG_SIZE: u64 = 24;
const OUTPUT_SIZE: usize = 28;
const INPUT_SIZE: usize = 16;
const DEFAULT_VERSION: u32 = 2;
const DEFAULT_PROG: u16 = 0;
const DEFAULT_PORT: u16 = 0;
const DEFAULT_EXPORT_NAME: &str = "default_name";
const DEFAULT_ADDRESS: &str = "0.0.0.0:111";

#[async_trait]
impl vfs::NFSFileSystem for DemoFS {
    fn generation(&self) -> u64 {
        unimplemented!()
    }

    fn capabilities(&self) -> Capabilities {
        unimplemented!()
    }

    fn root_dir(&self) -> fileid3 {
        unimplemented!()
    }

    async fn lookup(&self, _dirid: fileid3, _filename: &filename3) -> Result<fileid3, nfsstat3> {
        unimplemented!()
    }

    async fn getattr(&self, _id: fileid3) -> Result<fattr3, nfsstat3> {
        unimplemented!()
    }

    async fn setattr(&self, _id: fileid3, _setattr: sattr3) -> Result<fattr3, nfsstat3> {
        unimplemented!()
    }

    async fn read(
        &self,
        _id: fileid3,
        _offset: u64,
        _count: u32,
    ) -> Result<(Vec<u8>, bool), nfsstat3> {
        unimplemented!()
    }

    async fn write(&self, _id: fileid3, _offset: u64, _data: &[u8]) -> Result<fattr3, nfsstat3> {
        unimplemented!()
    }

    async fn create(
        &self,
        _dirid: fileid3,
        _filename: &filename3,
        _attr: sattr3,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
        unimplemented!()
    }

    async fn create_exclusive(
        &self,
        _dirid: fileid3,
        _filename: &filename3,
    ) -> Result<fileid3, nfsstat3> {
        unimplemented!()
    }

    async fn mkdir(
        &self,
        _dirid: fileid3,
        _dirname: &filename3,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
        unimplemented!()
    }

    async fn remove(&self, _dirid: fileid3, _filename: &filename3) -> Result<(), nfsstat3> {
        unimplemented!()
    }

    async fn rename(
        &self,
        _from_dirid: fileid3,
        _from_filename: &filename3,
        _to_dirid: fileid3,
        _to_filename: &filename3,
    ) -> Result<(), nfsstat3> {
        unimplemented!()
    }

    async fn readdir(
        &self,
        _dirid: fileid3,
        _start_after: fileid3,
        _max_entries: usize,
    ) -> Result<ReadDirResult, nfsstat3> {
        unimplemented!()
    }

    async fn symlink(
        &self,
        _dirid: fileid3,
        _linkname: &filename3,
        _symlink: &nfspath3,
        _attr: &sattr3,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
        unimplemented!()
    }

    async fn readlink(&self, _id: fileid3) -> Result<nfspath3, nfsstat3> {
        unimplemented!()
    }

    async fn link(
        &self,
        _file_id: fileid3,
        _link_dir_id: fileid3,
        _link_name: &filename3,
    ) -> Result<fattr3, nfsstat3> {
        unimplemented!()
    }

    async fn mknod(
        &self,
        _dir_id: fileid3,
        _name: &filename3,
        _ftype: ftype3,
        _specdata: specdata3,
        _attrs: &sattr3,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
        unimplemented!()
    }

    async fn commit(
        &self,
        _file_id: fileid3,
        _offset: u64,
        _count: u32,
    ) -> Result<fattr3, nfsstat3> {
        unimplemented!()
    }
}

fn multiple_mappings(amount: u32, prot: u32) -> Vec<mapping> {
    let mut result = Vec::<mapping>::with_capacity(amount as usize);
    for i in 1..=amount / 2 {
        result.push(mapping { prog: i, vers: 1, prot, port: i + 1000 });
        result.push(mapping { prog: i, vers: 2, prot, port: i + 2000 });
    }
    result
}
fn multiple_contexts(amount: u32) -> Vec<Context> {
    let mut result = Vec::<Context>::with_capacity(amount as usize);
    let table = Arc::from(RwLock::from(PortmapTable::default()));
    for i in 1..=amount {
        result.push(Context {
            local_port: DEFAULT_PROG,
            client_addr: format!("0.0.0.0:{}", i),
            auth: xdr::rpc::auth_unix::default(),
            vfs: Arc::new(DemoFS { _root: String::default() }),
            mount_signal: None,
            export_name: Arc::from(DEFAULT_EXPORT_NAME.to_string()),
            transaction_tracker: Arc::new(rpc::TransactionTracker::new(Duration::from_secs(60))),
            portmap_table: table.clone(),
        });
    }
    result
}

fn send_get_port(
    context: &mut Context,
    input: &mut Cursor<Vec<u8>>,
    output: &mut Cursor<Vec<u8>>,
    mapping_args: mapping,
) -> Result<(), anyhow::Error> {
    let body = call_body {
        rpcvers: DEFAULT_VERSION,
        prog: xdr::portmap::PROGRAM,
        vers: xdr::portmap::VERSION,
        proc: xdr::portmap::PortmapProgram::PMAPPROC_GETPORT.to_u32().unwrap(),
        cred: Default::default(),
        verf: Default::default(),
    };

    mapping_args.serialize(input)?;
    input.set_position(0);
    nfs_mamont::protocol::nfs::portmap::handle_portmap(
        u32::default(),
        &body,
        input,
        output,
        context,
    )
}

fn send_set_port(
    context: &mut Context,
    input: &mut Cursor<Vec<u8>>,
    output: &mut Cursor<Vec<u8>>,
    mapping_args: mapping,
) -> Result<(), anyhow::Error> {
    let body = call_body {
        rpcvers: DEFAULT_VERSION,
        prog: xdr::portmap::PROGRAM,
        vers: xdr::portmap::VERSION,
        proc: xdr::portmap::PortmapProgram::PMAPPROC_SET.to_u32().unwrap(),
        cred: Default::default(),
        verf: Default::default(),
    };
    mapping_args.serialize(input)?;

    input.set_position(0);
    nfs_mamont::protocol::nfs::portmap::handle_portmap(
        u32::default(),
        &body,
        input,
        output,
        context,
    )
}

fn send_unset_port(
    context: &mut Context,
    input: &mut Cursor<Vec<u8>>,
    output: &mut Cursor<Vec<u8>>,
    mapping_args: mapping,
) -> Result<(), anyhow::Error> {
    let body = call_body {
        rpcvers: DEFAULT_VERSION,
        prog: xdr::portmap::PROGRAM,
        vers: xdr::portmap::VERSION,
        proc: xdr::portmap::PortmapProgram::PMAPPROC_UNSET.to_u32().unwrap(),
        cred: Default::default(),
        verf: Default::default(),
    };

    mapping_args.serialize(input)?;
    input.set_position(0);
    nfs_mamont::protocol::nfs::portmap::handle_portmap(
        u32::default(),
        &body,
        input,
        output,
        context,
    )
}
/// Assisting function for RPC portmap operation (`GETPORT`, `SET`, or `UNSET`) to ease operations with Cursors and asserts
///
/// This function:
/// 1. Resets the input/output cursor positions (to ensure clean state).
/// 2. Executes the provided portmap operation (`function`) with the given arguments.
/// 3. Deserializes the output buffer into type `T` (expected result type).
/// 4. Compares the deserialized result with `expected`, panicking if they differ.
///
/// # Parameters
/// - `function`: One of the portmap operations (`send_get_port`, `send_set_port`, `send_unset_port`).
/// - `context`: Mutable NFS context (e.g., for tracking RPC transactions).
/// - `input`: Mutable cursor containing serialized input arguments (e.g., `mapping`).
/// - `output`: Mutable cursor where the RPC response will be written.
/// - `mapping`: Portmap arguments (program, port, protocol, etc.).
/// - `expected`: The expected deserialized result (e.g., `0` for failure, port number for success).
///
/// # Type Constraints
/// - `T` must be deserializable (via `xdr::Deserialize`), comparable (`PartialEq`),
///   and have a default value (`Default`). Used for the response type.
/// - `F`: A closure or function pointer matching one of the portmap operations.
fn call_assert<F, T>(
    function: F,
    context: &mut Context,
    input: &mut Cursor<Vec<u8>>,
    output: &mut Cursor<Vec<u8>>,
    mapping: mapping,
    expected: T,
) where
    F: FnOnce(
        &mut Context,
        &mut Cursor<Vec<u8>>,
        &mut Cursor<Vec<u8>>,
        mapping,
    ) -> Result<(), anyhow::Error>,
    T: PartialEq + Default + xdr::Deserialize + std::fmt::Debug,
{
    input.set_position(0);
    output.set_position(0);
    function(context, input, output, mapping).expect("can't proceed operation");
    output.set_position(RPC_MSG_SIZE);
    let res = deserialize::<T>(output).expect("can't get result");
    assert_eq!(res, expected);
}

#[cfg(test)]
mod tests {
    use super::*;
    /// simple test to assure, that result of GET_PORT operation is zero,
    /// when there is no attached port to corresponding program
    fn get_port_zero_reply(port: u16) {
        let mut context = Context {
            local_port: DEFAULT_PORT,
            client_addr: DEFAULT_ADDRESS.to_string(),
            auth: xdr::rpc::auth_unix::default(),
            vfs: Arc::new(DemoFS { _root: String::default() }),
            mount_signal: None,
            export_name: Arc::from(DEFAULT_EXPORT_NAME.to_string()),
            transaction_tracker: Arc::new(rpc::TransactionTracker::new(Duration::from_secs(60))),
            portmap_table: Arc::from(RwLock::from(PortmapTable::default())),
        };
        let mut input = Cursor::new(Vec::with_capacity(INPUT_SIZE));
        let mut output = Cursor::new(Vec::with_capacity(OUTPUT_SIZE));
        let mapping_args = mapping {
            prog: nfs3::PROGRAM,
            vers: DEFAULT_VERSION,
            prot: IPPROTO_TCP,
            port: port as u32,
        };
        call_assert(send_get_port, &mut context, &mut input, &mut output, mapping_args, 0);
    }

    ///simple test to assure, that after SET_PORT operation for program without
    /// associated port, entry creates and result of operation is TRUE
    fn set_port_ok_reply(port: u16) {
        let mut context = Context {
            local_port: DEFAULT_PORT,
            client_addr: DEFAULT_ADDRESS.to_string(),
            auth: xdr::rpc::auth_unix::default(),
            vfs: Arc::new(DemoFS { _root: String::default() }),
            mount_signal: None,
            export_name: Arc::from(DEFAULT_EXPORT_NAME.to_string()),
            transaction_tracker: Arc::new(rpc::TransactionTracker::new(Duration::from_secs(60))),
            portmap_table: Arc::from(RwLock::from(PortmapTable::default())),
        };
        let mut input = Cursor::new(Vec::with_capacity(INPUT_SIZE));
        let mut output = Cursor::new(Vec::with_capacity(OUTPUT_SIZE));
        let mapping_args = mapping {
            prog: nfs3::PROGRAM,
            vers: DEFAULT_VERSION,
            prot: IPPROTO_TCP,
            port: port as u32,
        };
        call_assert(send_get_port, &mut context, &mut input, &mut output, mapping_args, 0);
        call_assert(send_set_port, &mut context, &mut input, &mut output, mapping_args, true);
    }

    ///simple test of GET_PORT after SET_PORT
    fn get_port_ok_reply(port: u16) {
        let mapping_args = mapping {
            prog: nfs3::PROGRAM,
            vers: DEFAULT_VERSION,
            prot: IPPROTO_TCP,
            port: port as u32,
        };
        let mut context = Context {
            local_port: DEFAULT_PORT,
            client_addr: DEFAULT_ADDRESS.to_string(),
            auth: xdr::rpc::auth_unix::default(),
            vfs: Arc::new(DemoFS { _root: String::default() }),
            mount_signal: None,
            export_name: Arc::from(DEFAULT_EXPORT_NAME.to_string()),
            transaction_tracker: Arc::new(rpc::TransactionTracker::new(Duration::from_secs(60))),
            portmap_table: Arc::from(RwLock::from(PortmapTable::default())),
        };
        let mut input = Cursor::new(Vec::with_capacity(INPUT_SIZE));
        let mut output = Cursor::new(Vec::with_capacity(OUTPUT_SIZE));
        call_assert(send_set_port, &mut context, &mut input, &mut output, mapping_args, true);
        call_assert(
            send_get_port,
            &mut context,
            &mut input,
            &mut output,
            mapping_args,
            port as u32,
        );
    }

    ///test of multiple GET_PORT after SET_PORT
    fn set_and_get_multiple(amount: u32) {
        let maps = multiple_mappings(amount, IPPROTO_TCP);
        let mut context = Context {
            local_port: DEFAULT_PORT,
            client_addr: DEFAULT_ADDRESS.to_string(),
            auth: xdr::rpc::auth_unix::default(),
            vfs: Arc::new(DemoFS { _root: String::default() }),
            mount_signal: None,
            export_name: Arc::from(DEFAULT_EXPORT_NAME.to_string()),
            transaction_tracker: Arc::new(rpc::TransactionTracker::new(Duration::from_secs(60))),
            portmap_table: Arc::from(RwLock::from(PortmapTable::default())),
        };
        let mut input = Cursor::new(Vec::with_capacity(INPUT_SIZE));
        let mut output = Cursor::new(Vec::with_capacity(OUTPUT_SIZE));

        for mapping_arg in maps.clone() {
            call_assert(send_set_port, &mut context, &mut input, &mut output, mapping_arg, true);
        }

        for mapping_arg in maps {
            call_assert(
                send_get_port,
                &mut context,
                &mut input,
                &mut output,
                mapping_arg,
                mapping_arg.prog + mapping_arg.vers * 1000,
            );
        }
    }

    ///test of multiple operations asynchronously
    fn multi_thread_get_set(amount: usize) {
        let mut contexts = multiple_contexts((amount / 2) as u32);
        let mappings = multiple_mappings(amount as u32, IPPROTO_TCP);

        let mut set_for_thread = Vec::with_capacity(amount / 2);
        for i in 0..amount / 2 {
            set_for_thread.push((contexts[i].clone(), (mappings[i], mappings[amount / 2 + i])));
        }

        std::thread::scope(|scope| {
            for (mut context, (mappings_1, mappings_2)) in set_for_thread {
                scope.spawn(move || {
                    let mut input = Cursor::new(Vec::with_capacity(INPUT_SIZE));
                    let mut output = Cursor::new(Vec::with_capacity(OUTPUT_SIZE));
                    call_assert(
                        send_get_port,
                        &mut context,
                        &mut input,
                        &mut output,
                        mappings_1,
                        0,
                    );
                    call_assert(
                        send_set_port,
                        &mut context,
                        &mut input,
                        &mut output,
                        mappings_1,
                        true,
                    );
                    call_assert(
                        send_set_port,
                        &mut context,
                        &mut input,
                        &mut output,
                        mappings_2,
                        true,
                    );
                    call_assert(
                        send_get_port,
                        &mut context,
                        &mut input,
                        &mut output,
                        mappings_1,
                        mappings_1.prog + mappings_1.vers * 1000,
                    );
                    call_assert(
                        send_get_port,
                        &mut context,
                        &mut input,
                        &mut output,
                        mappings_2,
                        mappings_2.prog + mappings_2.vers * 1000,
                    );
                });
            }
        });

        let mut input = Cursor::new(Vec::with_capacity(INPUT_SIZE));
        let mut output = Cursor::new(Vec::with_capacity(OUTPUT_SIZE));

        for mapping_arg in mappings {
            call_assert(
                send_get_port,
                &mut contexts[0],
                &mut input,
                &mut output,
                mapping_arg,
                mapping_arg.prog + mapping_arg.vers * 1000,
            );
        }
    }
    ///test of UNSET when programs that haven't been mapped to port
    fn unset_empty_table(amount: u32) {
        let mut context = Context {
            local_port: DEFAULT_PORT,
            client_addr: DEFAULT_ADDRESS.to_string(),
            auth: xdr::rpc::auth_unix::default(),
            vfs: Arc::new(DemoFS { _root: String::default() }),
            mount_signal: None,
            export_name: Arc::from(DEFAULT_EXPORT_NAME.to_string()),
            transaction_tracker: Arc::new(rpc::TransactionTracker::new(Duration::from_secs(60))),
            portmap_table: Arc::from(RwLock::from(PortmapTable::default())),
        };
        let mut input = Cursor::new(Vec::with_capacity(INPUT_SIZE));
        let mut output = Cursor::new(Vec::with_capacity(OUTPUT_SIZE));

        let args_tcp = multiple_mappings(amount, IPPROTO_TCP);

        for arg in args_tcp {
            call_assert(send_unset_port, &mut context, &mut input, &mut output, arg, false);
        }
    }

    ///test of UNSET, when only one of two (TCP or UDP) protocols are mapped
    fn unset_single_protocol(amount: u32) {
        let mut context = Context {
            local_port: DEFAULT_PORT,
            client_addr: DEFAULT_ADDRESS.to_string(),
            auth: xdr::rpc::auth_unix::default(),
            vfs: Arc::new(DemoFS { _root: String::default() }),
            mount_signal: None,
            export_name: Arc::from(DEFAULT_EXPORT_NAME.to_string()),
            transaction_tracker: Arc::new(rpc::TransactionTracker::new(Duration::from_secs(60))),
            portmap_table: Arc::from(RwLock::from(PortmapTable::default())),
        };
        let mut input = Cursor::new(Vec::with_capacity(INPUT_SIZE));
        let mut output = Cursor::new(Vec::with_capacity(OUTPUT_SIZE));

        let args = multiple_mappings(amount, IPPROTO_UDP);

        for arg in &args {
            call_assert(send_set_port, &mut context, &mut input, &mut output, *arg, true);
        }

        for arg in args {
            call_assert(send_unset_port, &mut context, &mut input, &mut output, arg, true);
        }
    }

    ///test of UNSET, when both protocols (TCP or UDP) are mapped
    fn unset_both_protocols(amount: u32) {
        let mut context = Context {
            local_port: DEFAULT_PORT,
            client_addr: DEFAULT_ADDRESS.to_string(),
            auth: xdr::rpc::auth_unix::default(),
            vfs: Arc::new(DemoFS { _root: String::default() }),
            mount_signal: None,
            export_name: Arc::from(DEFAULT_EXPORT_NAME.to_string()),
            transaction_tracker: Arc::new(rpc::TransactionTracker::new(Duration::from_secs(60))),
            portmap_table: Arc::from(RwLock::from(PortmapTable::default())),
        };
        let mut input = Cursor::new(Vec::with_capacity(INPUT_SIZE));
        let mut output = Cursor::new(Vec::with_capacity(OUTPUT_SIZE));

        let args_udp = multiple_mappings(amount, IPPROTO_UDP);
        let args_tcp = multiple_mappings(amount, IPPROTO_UDP);

        for arg in &args_udp {
            call_assert(send_set_port, &mut context, &mut input, &mut output, *arg, true);
        }

        for arg in &args_tcp {
            call_assert(send_set_port, &mut context, &mut input, &mut output, *arg, true);
        }

        for mapping in args_tcp {
            call_assert(send_unset_port, &mut context, &mut input, &mut output, mapping, true);
        }
        for mapping in args_udp {
            call_assert(send_unset_port, &mut context, &mut input, &mut output, mapping, false);
        }
    }

    ///test of UNSET, where requests are sent from different threads
    fn unset_several_threads(amount_threads: usize) {
        let context = multiple_contexts(amount_threads as u32);
        let mapping_tcp = multiple_mappings(amount_threads as u32, IPPROTO_TCP);
        let mapping_udp = multiple_mappings(amount_threads as u32, IPPROTO_UDP);

        let mut set_for_thread: Vec<(Context, mapping, mapping)> =
            Vec::with_capacity(amount_threads);
        for i in 0..amount_threads {
            set_for_thread.push((context[i].clone(), mapping_tcp[i], mapping_udp[i]));
        }

        std::thread::scope(|scope| {
            for (mut context, mappings_1, mappings_2) in set_for_thread {
                scope.spawn(move || {
                    let mut input = Cursor::new(Vec::with_capacity(INPUT_SIZE));
                    let mut output = Cursor::new(Vec::with_capacity(OUTPUT_SIZE));
                    call_assert(
                        send_unset_port,
                        &mut context,
                        &mut input,
                        &mut output,
                        mappings_2,
                        false,
                    );
                    call_assert(
                        send_set_port,
                        &mut context,
                        &mut input,
                        &mut output,
                        mappings_2,
                        true,
                    );
                    call_assert(
                        send_set_port,
                        &mut context,
                        &mut input,
                        &mut output,
                        mappings_1,
                        true,
                    );
                    call_assert(
                        send_unset_port,
                        &mut context,
                        &mut input,
                        &mut output,
                        mappings_1,
                        true,
                    );
                    call_assert(
                        send_unset_port,
                        &mut context,
                        &mut input,
                        &mut output,
                        mappings_2,
                        false,
                    );
                });
            }
        });
        let mut input = Cursor::new(Vec::with_capacity(INPUT_SIZE));
        let mut output = Cursor::new(Vec::with_capacity(OUTPUT_SIZE));
        for mapping in mapping_udp {
            call_assert(
                send_get_port,
                &mut context[0].clone(),
                &mut input,
                &mut output,
                mapping,
                0,
            );
        }
    }

    #[test]
    fn get_port_zero_reply_multiple() {
        get_port_zero_reply(0);
        get_port_zero_reply(u16::MAX);
    }

    #[test]
    fn set_port_ok_reply_multiple() {
        set_port_ok_reply(0);
        set_port_ok_reply(u16::MAX);
    }
    #[test]

    fn get_port_ok_reply_multiple() {
        get_port_ok_reply(0);
        get_port_ok_reply(u16::MAX);
    }
    #[test]
    fn multiple_gets_after_sets() {
        set_and_get_multiple(0);
        set_and_get_multiple(789);
    }
    #[test]
    fn multi_threads_gets_sets() {
        multi_thread_get_set(0);
        multi_thread_get_set(100);
    }

    #[test]
    fn empty_unsets() {
        unset_empty_table(0);
        unset_empty_table(200);
    }

    #[test]
    fn unset_one_protocol_entry() {
        unset_single_protocol(0);
        unset_single_protocol(200);
    }

    #[test]
    fn unset_two_protocol_entry() {
        unset_both_protocols(0);
        unset_single_protocol(750);
    }

    #[test]
    fn multi_thread_unset() {
        unset_several_threads(0);
        unset_several_threads(100);
    }
}
