use std::path::PathBuf;

use fernfs::tcp::{NFSTcp, NFSTcpListener};

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

    let mut require_privileged_source_port = true;
    let mut path: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--allow-unprivileged-source-port" => {
                require_privileged_source_port = false;
            }
            "--help" | "-h" => {
                eprintln!(
                    "Usage: mirror_fs [--allow-unprivileged-source-port] <DIRECTORY>\n\
                     \n\
                     Options:\n\
                       --allow-unprivileged-source-port  Allow client source ports >= 1024 (default: require privileged)\n\
                       -h, --help                    Show this help and exit"
                );
                return;
            }
            _ if arg.starts_with('-') => {
                eprintln!("Unknown flag: {arg}");
                eprintln!("Run with --help for usage.");
                std::process::exit(2);
            }
            _ => {
                if path.is_some() {
                    eprintln!("Unexpected extra argument: {arg}");
                    eprintln!("Run with --help for usage.");
                    std::process::exit(2);
                }
                path = Some(PathBuf::from(arg));
            }
        }
    }

    let path = path.expect("must supply directory to mirror");

    let fs = fs::MirrorFS::new(path);
    let mut listener = NFSTcpListener::bind(&format!("127.0.0.1:{HOSTPORT}"), fs)
        .await
        .unwrap();
    listener.require_privileged_source_port(require_privileged_source_port);
    listener.handle_forever().await.unwrap();
}
