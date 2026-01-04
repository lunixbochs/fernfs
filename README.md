# FernFS

[![License](https://img.shields.io/badge/license-BSD--3--Clause-blue.svg)](LICENSE)

A complete NFSv3 server implementation in Rust that allows you to export any custom file system over the network.

## Features

- **Complete NFSv3 Protocol**: Full implementation of all 21 procedures defined in RFC 1813
- **MOUNT Protocol**: Support for filesystem exports and mount operations
- **PORTMAP Protocol**: Service discovery support for compatibility
- **Async/Await**: Built on Tokio for high-performance asynchronous I/O
- **Virtual File System**: Clean abstraction layer for implementing custom backends
- **Cross-Platform**: Works on Linux, macOS, and Windows
- **Standards Compliant**: Follows RFC 1813 (NFSv3), RFC 5531 (RPC), and RFC 1832 (XDR)

## Quick Start

Add this to your `Cargo.toml`:

```toml
[dependencies]
fernfs = "0.0.0"
tokio = { version = "1.0", features = ["full"] }
```

## Examples

### Running the Demo File System

The demo example creates an in-memory file system with some sample files:

```bash
cargo run --example demofs
```

Then mount it:

**Linux:**
```bash
mkdir /mnt/nfs
sudo mount -o proto=tcp,port=11111,mountport=11111,nolock,addr=127.0.0.1 127.0.0.1:/ /mnt/nfs
```

**macOS:**
```bash
mkdir /mnt/nfs
sudo mount_nfs -o nolocks,vers=3,tcp,rsize=131072,port=11111,mountport=11111 localhost:/ /mnt/nfs
```

**Windows (Pro/Enterprise):**
```cmd
mount.exe -o anon,nolock,mtype=soft,fileaccess=6,casesensitive,lang=ansi,rsize=128,wsize=128,timeout=60,retry=2 \\127.0.0.1\\ X:
```

### FernFS CLI (Mirror File System)

The `fernfs` binary exports an existing directory over NFS:

```bash
cargo install fernfs
fernfs /path/to/directory
```

For local development builds:

```bash
cargo run --bin fernfs -- /path/to/directory
```

#### Command Line Options

- `-h, --host <HOST>` Bind host (default: 127.0.0.1)
- `-p, --port <PORT>` Bind port (default: 11111)
- `--allow-unprivileged-source-port` Allow client source ports >= 1024 (default: require privileged)
- `--help` Show help and exit

## Creating Your Own NFS Server

To create a custom NFS server, implement the `NFSFileSystem` trait:

```rust
use fernfs::{tcp::NFSTcpListener, vfs::NFSFileSystem};
use async_trait::async_trait;

struct MyFileSystem {
    // Your file system state
}

#[async_trait]
impl NFSFileSystem for MyFileSystem {
    fn capabilities(&self) -> fernfs::vfs::Capabilities {
        fernfs::vfs::Capabilities::ReadWrite
    }

    fn root_dir(&self) -> u64 {
        1 // Root directory file ID
    }

    async fn lookup(&self, dirid: u64, filename: &[u8]) -> Result<u64, fernfs::nfs::nfsstat3> {
        // Implement file lookup logic
        todo!()
    }

    async fn getattr(&self, id: u64) -> Result<fernfs::nfs::fattr3, fernfs::nfs::nfsstat3> {
        // Return file attributes
        todo!()
    }

    // Implement other required methods...
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fs = MyFileSystem::new();
    let listener = NFSTcpListener::bind("127.0.0.1:11111", fs).await?;
    listener.handle_forever().await?;
    Ok(())
}
```

## Architecture

The library is structured into several key components:

- **`vfs`**: Virtual File System trait that you implement for your storage backend
- **`tcp`**: TCP server that handles client connections and protocol dispatch
- **`protocol`**: Internal implementation of NFS, MOUNT, and PORTMAP protocols
- **`xdr`**: XDR (External Data Representation) encoding/decoding

## File System Interface

The `NFSFileSystem` trait provides a clean abstraction with these key concepts:

- **File IDs**: Every file/directory has a unique 64-bit identifier (like an inode number)
- **File Handles**: Opaque handles that include generation numbers for stale handle detection
- **Stateless Operations**: All operations are stateless and use file IDs for addressing
- **Async Support**: All operations are async for high performance

## Standards Compliance

This implementation follows these RFCs:

- [RFC 1813](https://datatracker.ietf.org/doc/html/rfc1813): NFS Version 3 Protocol Specification
- [RFC 5531](https://datatracker.ietf.org/doc/html/rfc5531): RPC: Remote Procedure Call Protocol Specification Version 2
- [RFC 1832](https://datatracker.ietf.org/doc/html/rfc1832): XDR: External Data Representation Standard
- [RFC 1833](https://datatracker.ietf.org/doc/html/rfc1833): Binding Protocols for ONC RPC Version 2
