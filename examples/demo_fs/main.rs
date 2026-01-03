use fernfs::tcp::{NFSTcp, NFSTcpListener};

/// Implements the core file system functionality
mod fs;
/// Defines the storage representation for file system entries
mod fs_contents;
/// Defines the structure for file system entry metadata and content
mod fs_entry;

/// Port number on which the NFS server will listen
const HOSTPORT: u32 = 11111;

/// Demo NFS server implementation using the fernfs library.
/// Shows how to create a simple in-memory file system that supports NFS operations.
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_writer(std::io::stderr)
        .init();

    println!("Starting NFS server on 0.0.0.0:{HOSTPORT}");
    println!("You can mount it with: sudo mount -o proto=tcp,port={HOSTPORT},mountport={HOSTPORT},nolock,addr=127.0.0.1 127.0.0.1:/ /mnt/nfs");

    let listener =
        NFSTcpListener::bind(&format!("0.0.0.0:{HOSTPORT}"), fs::DemoFS::default()).await.unwrap();
    listener.handle_forever().await.unwrap();
}
