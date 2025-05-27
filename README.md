# NFS Mamont

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
nfs-mamont = "0.0.0"
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

### Mirror File System

The mirror example exports an existing directory over NFS:

```bash
cargo run --example mirrorfs /path/to/directory
```

## Creating Your Own NFS Server

To create a custom NFS server, implement the `NFSFileSystem` trait:

```rust
use nfs_mamont::{tcp::NFSTcpListener, vfs::NFSFileSystem};
use async_trait::async_trait;

struct MyFileSystem {
    // Your file system state
}

#[async_trait]
impl NFSFileSystem for MyFileSystem {
    fn capabilities(&self) -> nfs_mamont::vfs::Capabilities {
        nfs_mamont::vfs::Capabilities::ReadWrite
    }

    fn root_dir(&self) -> u64 {
        1 // Root directory file ID
    }

    async fn lookup(&self, dirid: u64, filename: &[u8]) -> Result<u64, nfs_mamont::nfs::nfsstat3> {
        // Implement file lookup logic
        todo!()
    }

    async fn getattr(&self, id: u64) -> Result<nfs_mamont::nfs::fattr3, nfs_mamont::nfs::nfsstat3> {
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

### Key Methods

- `lookup()`: Resolve filename to file ID within a directory
- `getattr()`/`setattr()`: Get/set file attributes
- `read()`/`write()`: File I/O operations
- `readdir()`: Directory listing with pagination
- `create()`/`mkdir()`/`remove()`: File system modifications
- `symlink()`/`readlink()`: Symbolic link operations

## Use Cases

This library is perfect for:

- **Remote File Systems**: Export cloud storage, databases, or APIs as file systems
- **Development Tools**: Create virtual file systems for testing and development
- **Cross-Platform Mounting**: Provide file access across different operating systems
- **Custom Storage**: Expose specialized storage systems through a standard interface

## Performance Considerations

- Implement `getattr()` efficiently as it's called frequently
- Use appropriate caching strategies in your file system implementation
- Consider the NFS client's caching behavior when designing your backend
- The server supports concurrent connections and operations

## Standards Compliance

This implementation follows these RFCs:

- [RFC 1813](https://datatracker.ietf.org/doc/html/rfc1813): NFS Version 3 Protocol Specification
- [RFC 5531](https://datatracker.ietf.org/doc/html/rfc5531): RPC: Remote Procedure Call Protocol Specification Version 2
- [RFC 1832](https://datatracker.ietf.org/doc/html/rfc1832): XDR: External Data Representation Standard
- [RFC 1833](https://datatracker.ietf.org/doc/html/rfc1833): Binding Protocols for ONC RPC Version 2

## CI/CD & Docker

This project uses GitFlic CI/CD for automated testing, security scanning, and deployment. The pipeline automatically runs on every push and merge request, ensuring code quality and reliability.

**Features**:
- 🔧 Automated Rust builds and testing
- 🐳 Docker containerization with multi-stage builds
- 🔒 Security scanning (dependencies + containers)
- 📦 Automatic artifact management
- 🚀 GitFlic container registry integration

**Quick Start**: Upload environment variables and push to `main` - everything else is pre-configured!

For complete setup instructions, see [`CI-CD.md`](CI-CD.md)

## Contributing

Contributions are welcome! Areas where help is particularly appreciated:

- Additional protocol features and edge cases
- Performance optimizations
- Documentation improvements
- Platform-specific testing and fixes
- Example implementations

### Development Workflow

1. Fork the repository
2. Create a feature branch from `main`
3. Make your changes with appropriate tests
4. Ensure all CI/CD checks pass
5. Submit a merge request

## License

This project is licensed under the BSD-3-Clause License - see the [LICENSE](LICENSE) file for details.
