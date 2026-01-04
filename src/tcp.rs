//! The TCP module provides functionality for handling NFS protocol communications over TCP.
//!
//! This module implements a TCP listener for NFS server that:
//! - Handles connections from NFS clients
//! - Processes RPC messages received over TCP
//! - Manages connection lifecycle and message framing
//! - Provides interface for mounting and unmounting file systems
//!
//! The implementation supports configurable export paths and notification
//! on mount/unmount operations.

use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use std::{io, net::IpAddr};

use anyhow;
use async_trait::async_trait;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::protocol::nfs::portmap::PortmapTable;
use crate::protocol::{rpc, xdr};
use crate::vfs::NFSFileSystem;

/// NFS TCP Connection Handler that listens for incoming NFS client connections
/// and processes RPC messages over TCP transport.
pub struct NFSTcpListener<T: NFSFileSystem + Send + Sync + 'static> {
    /// TCP Listener for accepting incoming connections
    listener: TcpListener,
    /// Port on which the server is listening
    port: u16,
    /// Arc reference to the NFS file system implementation
    arcfs: Arc<T>,
    /// Optional channel for sending mount/unmount notifications
    mount_signal: Option<mpsc::Sender<bool>>,
    /// Name of the exported file system path
    export_name: Arc<String>,
    /// Tracker for RPC transactions to handle retransmissions
    transaction_tracker: Arc<rpc::TransactionTracker>,
    /// Portmap table storing port-to-program mappings
    /// (like a portmap service)
    portmap_table: Arc<RwLock<PortmapTable>>,
    /// Whether to require clients to use a privileged source port (< 1024)
    require_privileged_source_port: bool,
}

/// Generates a local loopback IP address from a 16-bit host number
/// Used for creating multiple local test addresses in the 127.88.x.y range
pub fn generate_host_ip(hostnum: u16) -> String {
    format!("127.88.{}.{}", ((hostnum >> 8) & 0xFF) as u8, (hostnum & 0xFF) as u8)
}

/// Processes an established TCP socket connection from an NFS client
///
/// This function:
/// - Creates an RPC message handler for the socket
/// - Sets up asynchronous message processing
/// - Handles bidirectional communication between client and server
/// - Processes incoming RPC requests and sends responses
///
/// # Arguments
///
/// * `socket` - The established TCP connection to the client
/// * `context` - RPC context containing server state and client information
async fn process_socket(
    mut socket: tokio::net::TcpStream,
    context: rpc::Context,
) -> Result<(), anyhow::Error> {
    let (mut message_handler, mut socksend, mut msgrecvchan) =
        rpc::SocketMessageHandler::new(&context);
    let _ = socket.set_nodelay(true);

    tokio::spawn(async move {
        loop {
            if let Err(e) = message_handler.read().await {
                debug!("Message loop broken due to {:?}", e);
                break;
            }
        }
    });
    loop {
        tokio::select! {
            _ = socket.readable() => {
                let mut buf = [0; 128_000];

                match socket.try_read(&mut buf) {
                    Ok(0) => {
                        return Ok(());
                    }
                    Ok(n) => {
                        let _ = socksend.write_all(&buf[..n]).await;
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        // do nothing
                    }
                    Err(e) => {
                        debug!("Message handling closed : {:?}", e);
                        return Err(e.into());
                    }
                }

            },
            reply = msgrecvchan.recv() => {
                match reply {
                    Some(Err(e)) => {
                        debug!("Message handling closed : {:?}", e);
                        return Err(e);
                    }
                    Some(Ok(msg)) => {
                        if let Err(e) = rpc::write_fragment(&mut socket, &msg).await {
                            error!("Write error {:?}", e);
                        }
                    }
                    None => {
                        return Err(anyhow::anyhow!("Unexpected socket context termination"));
                    }
                }
            }
        }
    }
}

/// Interface for NFS TCP servers that defines common operations
/// for managing and interacting with NFS clients over TCP connections.
///
/// This trait provides methods for:
/// - Getting information about the listening socket
/// - Setting up mount event notifications
/// - Starting the server to process client connections
#[async_trait]
pub trait NFSTcp: Send + Sync {
    /// Returns the actual port number on which the server is listening
    ///
    /// This is especially useful when binding to port 0, which allows the OS
    /// to assign any available port. After binding, this method can be used
    /// to determine which port was actually assigned.
    fn get_listen_port(&self) -> u16;

    /// Returns the IP address on which the server is listening
    ///
    /// This is useful when the server binds to a wildcard address (0.0.0.0 or ::)
    /// or when using the "auto" IP address feature, to determine the actual
    /// network interface being used.
    fn get_listen_ip(&self) -> IpAddr;

    /// Registers a channel to receive notifications about mount and unmount events
    ///
    /// # Arguments
    ///
    /// * `signal` - MPSC sender that will receive boolean values:
    ///   * `true` when a client mounts the file system
    ///   * `false` when a client unmounts the file system
    fn set_mount_listener(&mut self, signal: mpsc::Sender<bool>);

    /// Starts the NFS server and processes client connections
    ///
    /// This method:
    /// - Accepts incoming TCP connections from NFS clients
    /// - Creates a new RPC context for each connection
    /// - Spawns an asynchronous task to handle each connection
    /// - Continues accepting connections indefinitely
    ///
    /// This method runs in an infinite loop and only returns if there's an error
    /// with the underlying TCP listener.
    async fn handle_forever(&self) -> io::Result<()>;
}

impl<T: NFSFileSystem + Send + Sync + 'static> NFSTcpListener<T> {
    /// Creates a new NFS TCP listener bound to the specified IP address and port
    ///
    /// # Arguments
    ///
    /// * `ipstr` - IP address and port in the format "IP:PORT" (e.g. "127.0.0.1:2049")
    ///   Special value "auto:PORT" attempts to find an available local address
    /// * `fs` - Implementation of the [`NFSFileSystem`] trait that will handle NFS operations
    ///
    /// # Returns
    ///
    /// A Result containing either the new [`NFSTcpListener`] or an IO error
    pub async fn bind(ipstr: &str, fs: T) -> io::Result<NFSTcpListener<T>> {
        let arcfs: Arc<T> = Arc::new(fs);
        if let Some(port_str) = ipstr.strip_prefix("auto:") {
            let port = port_str.parse::<u16>().map_err(|_| {
                io::Error::new(io::ErrorKind::AddrNotAvailable, "Port not in range 0..=65535")
            })?;

            const NUM_TRIES: u16 = 32;
            for try_ip in 1..=NUM_TRIES {
                let ip: IpAddr = generate_host_ip(try_ip).parse().map_err(|_| {
                    io::Error::new(io::ErrorKind::AddrNotAvailable, "Invalid auto IP address")
                })?;
                let addr = SocketAddr::new(ip, port);
                let result = NFSTcpListener::bind_internal(addr, arcfs.clone()).await;

                if result.is_ok() {
                    return result;
                }
            }

            return Err(io::Error::other("Can't bind automatically"));
        }

        let addr = ipstr.parse::<SocketAddr>().map_err(|_| {
            io::Error::new(
                io::ErrorKind::AddrNotAvailable,
                "Address must be in IP:PORT or [IPV6]:PORT form",
            )
        })?;
        NFSTcpListener::bind_internal(addr, arcfs).await
    }

    /// Internal method to bind the TCP listener to a specific IP and port
    ///
    /// # Arguments
    ///
    /// * `ip` - IP address to bind to
    /// * `port` - Port number to bind to
    /// * `arcfs` - Arc reference to the NFS file system implementation
    async fn bind_internal(addr: SocketAddr, arcfs: Arc<T>) -> io::Result<NFSTcpListener<T>> {
        let listener = TcpListener::bind(addr).await?;
        let local_addr = listener.local_addr()?;
        info!("Listening on {:?}", local_addr);
        let port = local_addr.port();
        Ok(NFSTcpListener {
            listener,
            port,
            arcfs,
            mount_signal: None,
            export_name: Arc::from("/".to_string()),
            transaction_tracker: Arc::new(rpc::TransactionTracker::new(Duration::from_secs(60))),
            portmap_table: Arc::from(RwLock::from(PortmapTable::default())),
            require_privileged_source_port: false,
        })
    }

    /// Sets an optional NFS export name.
    ///
    /// The export name defines the path that clients will use to mount the file system.
    /// This method normalizes the provided name by adding a leading slash and removing
    /// any trailing slashes.
    ///
    /// # Arguments
    ///
    /// * `export_name`: The desired export name without slashes.
    pub fn with_export_name<S: AsRef<str>>(&mut self, export_name: S) {
        self.export_name = Arc::new(format!(
            "/{}",
            export_name.as_ref().trim_end_matches('/').trim_start_matches('/')
        ));
    }

    /// Require clients to use a privileged source port (< 1024) when connecting.
    pub fn require_privileged_source_port(&mut self, require: bool) {
        self.require_privileged_source_port = require;
    }
}

#[async_trait]
impl<T: NFSFileSystem + Send + Sync + 'static> NFSTcp for NFSTcpListener<T> {
    /// Returns the actual port number on which the server is listening
    ///
    /// This is especially useful when binding to port 0, which allows the OS
    /// to assign any available port. After binding, this method can be used
    /// to determine which port was actually assigned.
    fn get_listen_port(&self) -> u16 {
        let addr = self.listener.local_addr().unwrap();
        addr.port()
    }

    /// Returns the IP address on which the server is listening
    ///
    /// This is useful when the server binds to a wildcard address (0.0.0.0 or ::)
    /// or when using the "auto" IP address feature, to determine the actual
    /// network interface being used.
    fn get_listen_ip(&self) -> IpAddr {
        let addr = self.listener.local_addr().unwrap();
        addr.ip()
    }

    /// Registers a channel to receive notifications about mount and unmount events
    ///
    /// # Arguments
    ///
    /// * `signal` - MPSC sender that will receive boolean values:
    ///   * `true` when a client mounts the file system
    ///   * `false` when a client unmounts the file system
    fn set_mount_listener(&mut self, signal: mpsc::Sender<bool>) {
        self.mount_signal = Some(signal);
    }

    /// Starts the NFS server and processes client connections
    ///
    /// This method:
    /// - Accepts incoming TCP connections from NFS clients
    /// - Creates a new RPC context for each connection
    /// - Spawns an asynchronous task to handle each connection
    /// - Continues accepting connections indefinitely
    ///
    /// This method runs in an infinite loop and only returns if there's an error
    /// with the underlying TCP listener.
    async fn handle_forever(&self) -> io::Result<()> {
        loop {
            let (socket, peer_addr) = self.listener.accept().await?;
            if self.require_privileged_source_port && peer_addr.port() >= 1024 {
                warn!(
                    "Rejecting connection from {}: source port {} is not privileged",
                    peer_addr.ip(),
                    peer_addr.port()
                );
                continue;
            }
            let context = rpc::Context {
                local_port: self.port,
                client_addr: peer_addr.to_string(),
                auth: xdr::rpc::auth_unix::default(),
                vfs: self.arcfs.clone(),
                mount_signal: self.mount_signal.clone(),
                export_name: self.export_name.clone(),
                transaction_tracker: self.transaction_tracker.clone(),
                portmap_table: self.portmap_table.clone(),
            };
            info!("Accepting connection from {}", context.client_addr);
            debug!("Accepting socket {:?} {:?}", socket, context);
            tokio::spawn(async move {
                let _ = process_socket(socket, context).await;
            });
        }
    }
}
