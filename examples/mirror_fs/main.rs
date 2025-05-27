use std::path::PathBuf;

use nfs_mamont::tcp::{NFSTcp, NFSTcpListener};

const HOSTPORT: u32 = 11111;

pub mod create_fs_object;
pub mod error_handling;
pub mod fs;
pub mod fs_entry;
pub mod fs_map;

/// Main entry point for the mirror file system example
///
/// This function initializes the tracing subscriber, reads the directory path
/// from command line arguments, creates a MirrorFS instance, and starts
/// an NFS server on the specified port.
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_writer(std::io::stderr)
        .init();

    let path = std::env::args().nth(1).expect("must supply directory to mirror");
    let path = PathBuf::from(path);

    let fs = fs::MirrorFS::new(path);
    let listener = NFSTcpListener::bind(&format!("127.0.0.1:{HOSTPORT}"), fs).await.unwrap();
    listener.handle_forever().await.unwrap();
}
