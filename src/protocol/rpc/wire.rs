//! RPC message framing and transmission as specified in RFC 5531 (previously RFC 1057 section 10).
//!
//! This module implements the Record Marking Standard for sending RPC messages
//! over TCP connections. It provides:
//!
//! - Message fragmentation for large RPC messages
//! - Proper message delimitation in stream-oriented transports
//! - Asynchronous message processing
//! - RPC call dispatching to appropriate protocol handlers
//!
//! The wire protocol implementation handles all the low-level details of:
//! - Reading fragmentary messages and reassembling them
//! - Writing record-marked fragments with appropriate headers
//! - Managing socket communication channels
//! - Processing incoming RPC calls
//!
//! This module is essential for maintaining proper message boundaries in TCP
//! while providing efficient transmission of RPC messages of any size.

use std::io::Cursor;
use std::io::{Read, Write};
use std::sync::Arc;

use anyhow::anyhow;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::DuplexStream;
use tokio::sync::mpsc;
use tracing::{debug, error, trace, warn};

use crate::protocol::rpc::command_queue::{CommandQueue, CommandResult, ResponseBuffer};
use crate::protocol::xdr::{self, deserialize, mount, nfs3, portmap, Serialize};
use crate::protocol::{nfs, rpc};

// Information from RFC 5531 (ONC RPC v2)
// https://datatracker.ietf.org/doc/html/rfc5531
// Which obsoletes RFC 1831 and RFC 1057 (Original RPC)

/// RPC program number for NFS Access Control Lists
const NFS_ACL_PROGRAM: u32 = 100227;
/// RPC program number for NFS ID Mapping
const NFS_ID_MAP_PROGRAM: u32 = 100270;
/// RPC program number for LOCALIO auxiliary RPC protocol
/// More about <https://docs.kernel.org/filesystems/nfs/localio.html#rpc.>
const NFS_LOCALIO_PROGRAM: u32 = 400122;
/// RPC program number for NFS Metadata
const NFS_METADATA_PROGRAM: u32 = 200024;
/// Initial size of RPC response buffer
const DEFAULT_RESPONSE_BUFFER_CAPACITY: usize = 8192;

/// Outcome of handling an RPC message.
enum RpcOutcome {
    /// A response was written to the output buffer.
    Send { xid: u32, record_response: bool },
    /// No response should be sent (e.g. in-progress retransmission).
    Drop,
}

/// Processes a single RPC message
///
/// This function forms the core of the RPC message dispatcher. It:
/// 1. Deserializes the incoming RPC message using XDR format
/// 2. Validates the RPC version number (must be version 2)
/// 3. Extracts authentication information if provided
/// 4. Checks for retransmissions to ensure idempotent operation
/// 5. Routes the call to the appropriate protocol handler (NFS, MOUNT, PORTMAP)
/// 6. Tracks transaction completion state
///
/// This implementation follows RFC 5531 (previously RFC 1057) section on Authentication and
/// Record Marking Standard for proper RPC message handling.
///
/// Returns an outcome describing whether a response was sent or dropped.
async fn handle_rpc(
    input: &mut impl Read,
    output: &mut impl Write,
    mut context: rpc::Context,
) -> Result<RpcOutcome, anyhow::Error> {
    let recv = deserialize::<xdr::rpc::rpc_msg>(input)?;
    let xid = recv.xid;
    if let xdr::rpc::rpc_body::CALL(call) = recv.body {
        if let xdr::rpc::auth_flavor::AUTH_UNIX = call.cred.flavor {
            context.auth = deserialize(&mut Cursor::new(&call.cred.body))?;
        }
        let status = context.transaction_tracker.check(xid, &context.client_addr);
        match status {
            rpc::TransactionStatus::Completed(response) => {
                output.write_all(&response)?;
                return Ok(RpcOutcome::Send { xid, record_response: false });
            }
            rpc::TransactionStatus::InProgress => {
                debug!(
                    "Retransmission in progress, xid: {}, client_addr: {}, call: {:?}",
                    xid, context.client_addr, call
                );
                return Ok(RpcOutcome::Drop);
            }
            rpc::TransactionStatus::New => {}
        }
        if call.rpcvers != 2 {
            warn!("Invalid RPC version {} != 2", call.rpcvers);
            xdr::rpc::rpc_vers_mismatch(xid).serialize(output)?;
            return Ok(RpcOutcome::Send { xid, record_response: true });
        }

        let res = {
            match call.prog {
                nfs3::PROGRAM => match call.vers {
                    nfs3::VERSION => nfs::v3::handle_nfs(xid, call, input, output, &context).await,
                    _ => {
                        warn!(
                            "Unsupported NFS program version {} (supported {})",
                            call.vers,
                            nfs3::VERSION
                        );
                        xdr::rpc::prog_mismatch_reply_message(xid, nfs3::VERSION)
                            .serialize(output)?;
                        Ok(())
                    }
                },
                portmap::PROGRAM => {
                    nfs::portmap::handle_portmap(xid, &call, input, output, &mut context)
                }
                mount::PROGRAM => {
                    nfs::mount::handle_mount(xid, call, input, output, &context).await
                }
                NFS_ACL_PROGRAM | NFS_ID_MAP_PROGRAM | NFS_METADATA_PROGRAM => {
                    trace!("ignoring NFS_ACL packet");
                    xdr::rpc::prog_unavail_reply_message(xid).serialize(output)?;
                    Ok(())
                }
                NFS_LOCALIO_PROGRAM => {
                    trace!("Ignoring NFS_LOCALIO packet");
                    xdr::rpc::prog_unavail_reply_message(xid).serialize(output)?;
                    Ok(())
                }
                unknown_number => {
                    warn!("Unknown RPC Program number {} != {}", unknown_number, nfs3::PROGRAM);
                    xdr::rpc::prog_unavail_reply_message(xid).serialize(output)?;
                    Ok(())
                }
            }
        };
        match res {
            Ok(()) => Ok(RpcOutcome::Send { xid, record_response: true }),
            Err(e) => {
                context.transaction_tracker.clear(xid, &context.client_addr);
                Err(e)
            }
        }
    } else {
        error!("Unexpectedly received a Reply instead of a Call");
        Err(anyhow!("Bad RPC Call format"))
    }
}

/// Reads a single record-marked fragment from a stream
///
/// Implements the RFC 5531 (previously RFC 1057 section 10) Record Marking Standard for TCP transport.
/// The record marking standard addresses the problem of delimiting records in a
/// stream protocol like TCP by prefixing each record with a 4-byte header.
///
/// This function:
/// 1. Reads the 4-byte header from the socket
/// 2. Extracts the fragment length (lower 31 bits) and last-fragment flag (highest bit)
/// 3. Reads exactly that many bytes from the socket
/// 4. Appends the read data to the provided buffer
///
/// Returns true if this was the last fragment in the RPC record, false otherwise.
/// This allows for reassembly of multi-fragment RPC messages.
async fn read_fragment(
    socket: &mut DuplexStream,
    append_to: &mut Vec<u8>,
) -> Result<bool, anyhow::Error> {
    let mut header_buf = [0_u8; 4];
    socket.read_exact(&mut header_buf).await?;
    let fragment_header = u32::from_be_bytes(header_buf);
    let is_last = (fragment_header & (1 << 31)) > 0;
    let length = (fragment_header & ((1 << 31) - 1)) as usize;
    trace!("Reading fragment length:{}, last:{}", length, is_last);
    if append_to.len().saturating_add(length) > rpc::MAX_RPC_RECORD_LENGTH {
        return Err(anyhow!(
            "RPC record length {} exceeds max {}",
            length,
            rpc::MAX_RPC_RECORD_LENGTH
        ));
    }
    let start_offset = append_to.len();
    append_to.resize(append_to.len() + length, 0);
    socket.read_exact(&mut append_to[start_offset..]).await?;
    trace!("Finishing Reading fragment length:{}, last:{}", length, is_last);
    Ok(is_last)
}

/// Writes data as record-marked fragments to a TCP stream
///
/// Implements the RFC 5531 (previously RFC 1057 section 10) Record Marking Standard for TCP transport.
/// This standard enables RPC to utilize TCP as a transport while maintaining proper
/// message boundaries essential for RPC semantics.
///
/// The function:
/// 1. Divides large buffers into manageable fragments (maximum 2GB each)
/// 2. Prefixes each fragment with a 4-byte header
///    - The lower 31 bits contain the fragment length
///    - The highest bit indicates if this is the last fragment (1=last, 0=more)
/// 3. Writes both header and data to the socket
///
/// This ensures reliable transmission of RPC messages over TCP with proper
/// message framing and enables receivers to allocate appropriate buffer space.
pub async fn write_fragment(
    socket: &mut tokio::net::TcpStream,
    buf: &[u8],
) -> Result<(), anyhow::Error> {
    // Maximum fragment size is 2^31 - 1 bytes
    const MAX_FRAGMENT_SIZE: usize = (1 << 31) - 1;

    let mut offset = 0;
    while offset < buf.len() {
        // Calculate the size of this fragment
        let remaining = buf.len() - offset;
        let fragment_size = std::cmp::min(remaining, MAX_FRAGMENT_SIZE);

        // Determine if this is the last fragment
        let is_last = offset + fragment_size >= buf.len();

        // Create the fragment header
        // The highest bit indicates if this is the last fragment
        let fragment_header =
            if is_last { fragment_size as u32 + (1 << 31) } else { fragment_size as u32 };

        let header_buf = u32::to_be_bytes(fragment_header);
        socket.write_all(&header_buf).await?;

        trace!("Writing fragment length:{}, last:{}", fragment_size, is_last);
        socket.write_all(&buf[offset..offset + fragment_size]).await?;

        offset += fragment_size;
    }

    Ok(())
}

pub type SocketMessageType = Result<Vec<u8>, anyhow::Error>;

/// Handles RPC message processing over a TCP connection
///
/// Receives record-marked RPC messages from a TCP stream, processes
/// them asynchronously by dispatching to the appropriate protocol handlers,
/// and manages the response flow. Implements the record marking protocol
/// for reliable message delimitation over TCP.
#[derive(Debug)]
pub struct SocketMessageHandler {
    /// Buffer for current fragment
    cur_fragment: Vec<u8>,
    /// Channel for receiving data from socket
    socket_receive_channel: DuplexStream,
    /// RPC context for request processing
    context: rpc::Context,
    /// Command queue for ordered processing
    command_queue: CommandQueue,
}

impl SocketMessageHandler {
    /// Creates a new `SocketMessageHandler` instance
    ///
    /// Initializes the handler with the provided RPC context and creates the
    /// necessary communication channels. Returns the handler itself, a duplex
    /// stream for writing to the socket, and a receiver for processed messages.
    ///
    /// This setup enables asynchronous processing of RPC messages while maintaining
    /// order of operations.
    pub fn new(
        context: &rpc::Context,
    ) -> (Self, DuplexStream, mpsc::UnboundedReceiver<SocketMessageType>) {
        let (socksend, sockrecv) = tokio::io::duplex(256_000);
        let (msgsend, msgrecv) = mpsc::unbounded_channel();

        // Create separate channel for command results
        let (result_sender, mut result_receiver) = mpsc::unbounded_channel::<CommandResult>();

        // Create command queue with our RPC processing function
        let command_queue =
            CommandQueue::new(process_rpc_command, result_sender, DEFAULT_RESPONSE_BUFFER_CAPACITY);

        // Process results from command queue and send them to socket
        tokio::spawn(async move {
            while let Some(result) = result_receiver.recv().await {
                match result {
                    Ok(Some(response_buffer)) if response_buffer.has_content() => {
                        let _ = msgsend.send(Ok(response_buffer.into_inner()));
                    }
                    Ok(None) => {
                        // No response needed, so nothing to send
                    }
                    Ok(Some(_)) => {
                        // Buffer exists but contains no data to send
                    }
                    Err(e) => {
                        error!("RPC error: {:?}", e);
                        let _ = msgsend.send(Err(e));
                    }
                }
            }
            debug!("Command result handler finished");
        });

        (
            Self {
                cur_fragment: Vec::new(),
                socket_receive_channel: sockrecv,
                context: context.clone(),
                command_queue,
            },
            socksend,
            msgrecv,
        )
    }

    /// Reads and processes a fragment from the socket
    ///
    /// Reads a single record-marked fragment from the socket and appends it to
    /// the current message buffer. If the fragment is the last one in the record,
    /// submits a command to the queue for processing in order.
    /// Should be called in a loop to continuously process incoming messages.
    pub async fn read(&mut self) -> Result<(), anyhow::Error> {
        let is_last =
            read_fragment(&mut self.socket_receive_channel, &mut self.cur_fragment).await?;
        if is_last {
            // Take buffer and create new one for next fragment
            let fragment_data = std::mem::take(&mut self.cur_fragment);
            let context = self.context.clone();

            // Submit command to queue for ordered processing
            if let Err(e) = self.command_queue.submit_command(fragment_data, context) {
                error!("Failed to submit command to queue: {:?}", e);
                return Err(anyhow::anyhow!("Command queue error: {}", e));
            }
        }
        Ok(())
    }
}

/// Standard async RPC processing function that can be used with `CommandQueue`
///
/// Processes an RPC command by:
/// 1. Deserializing the RPC message
/// 2. Processing the RPC call according to standard protocol
/// 3. Writing response to output buffer
///
/// # Arguments
///
/// * `data` - Buffer containing RPC message
/// * `output` - Buffer for writing response
/// * `context` - RPC processing context
///
/// # Returns
///
/// `Ok(true)` if response needs to be sent
/// `Ok(false)` if no response needed (e.g. retransmission)
/// `Err` if processing error occurred
pub fn process_rpc_command<'a>(
    data: &[u8],
    output: &'a mut ResponseBuffer,
    context: rpc::Context,
) -> futures::future::BoxFuture<'a, anyhow::Result<bool>> {
    // Clone data to own it in closure
    let data_clone = data.to_vec();

    Box::pin(async move {
        // Create cursor for reading data
        let mut input_cursor = Cursor::new(data_clone);

        // Get internal buffer for writing
        let output_buffer = output.get_mut_buffer();
        let mut output_cursor = Cursor::new(&mut *output_buffer);

        let tracker = context.transaction_tracker.clone();
        let client_addr = context.client_addr.clone();
        // Call RPC handler
        let result = handle_rpc(&mut input_cursor, &mut output_cursor, context).await?;

        match result {
            RpcOutcome::Send { xid, record_response } => {
                if record_response && !output_buffer.is_empty() {
                    tracker.record_response(xid, &client_addr, Arc::new(output_buffer.clone()));
                }
                Ok(true)
            }
            RpcOutcome::Drop => Ok(false),
        }
    })
}
