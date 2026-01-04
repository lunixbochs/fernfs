use std::path::PathBuf;

use fernfs::tcp::{NFSTcp, NFSTcpListener};

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 11111;

pub mod create_fs_object;
pub mod error_handling;
pub mod fs;
pub mod fs_entry;
pub mod fs_map;

/// Main entry point for the FernFS CLI (mirror file system)
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

    fn print_help() {
        eprintln!(
            "Usage: fernfs [--host <HOST>] [--port <PORT>] [--allow-unprivileged-source-port] <DIRECTORY>\n\
             \n\
             Options:\n\
               -h, --host <HOST>                Bind host (default: {DEFAULT_HOST})\n\
               -p, --port <PORT>                Bind port (default: {DEFAULT_PORT})\n\
               --allow-unprivileged-source-port Allow client source ports >= 1024 (default: require privileged)\n\
               --help                           Show this help and exit"
        );
    }

    fn require_value(flag: &str, args: &mut impl Iterator<Item = String>) -> String {
        args.next().unwrap_or_else(|| {
            eprintln!("Missing value for {flag}");
            eprintln!("Run with --help for usage.");
            std::process::exit(2);
        })
    }

    fn parse_host(value: &str) -> String {
        if value.is_empty() {
            eprintln!("Host cannot be empty");
            eprintln!("Run with --help for usage.");
            std::process::exit(2);
        }
        value.to_string()
    }

    fn parse_port(value: &str) -> u16 {
        value.parse::<u16>().unwrap_or_else(|_| {
            eprintln!("Invalid port: {value}");
            eprintln!("Run with --help for usage.");
            std::process::exit(2);
        })
    }

    let mut require_privileged_source_port = true;
    let mut host = DEFAULT_HOST.to_string();
    let mut port = DEFAULT_PORT;
    let mut path: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--allow-unprivileged-source-port" => {
                require_privileged_source_port = false;
            }
            "--host" | "-h" => {
                let value = require_value("--host", &mut args);
                host = parse_host(&value);
            }
            "--port" | "-p" => {
                let value = require_value("--port", &mut args);
                port = parse_port(&value);
            }
            "--help" => {
                print_help();
                return;
            }
            _ if arg.starts_with("--host=") => {
                let value = &arg["--host=".len()..];
                host = parse_host(value);
            }
            _ if arg.starts_with("-h=") => {
                let value = &arg["-h=".len()..];
                host = parse_host(value);
            }
            _ if arg.starts_with("--port=") => {
                let value = &arg["--port=".len()..];
                port = parse_port(value);
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

    let Some(path) = path else {
        print_help();
        std::process::exit(2);
    };

    let fs = fs::MirrorFS::new(path);
    let bind_addr = if host.contains(':') {
        if host.starts_with('[') && host.ends_with(']') {
            format!("{host}:{port}")
        } else {
            format!("[{host}]:{port}")
        }
    } else {
        format!("{host}:{port}")
    };
    let mut listener = NFSTcpListener::bind(&bind_addr, fs).await.unwrap();
    listener.require_privileged_source_port(require_privileged_source_port);
    listener.handle_forever().await.unwrap();
}
