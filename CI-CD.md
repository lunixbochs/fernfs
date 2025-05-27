# CI/CD Setup for NFS Mamont

## 🚀 Quick Start

This project uses GitFlic CI/CD with a **single `main` branch** workflow. Everything is pre-configured - just upload variables and start coding!

## ✅ What's Already Configured

✅ **Pipeline Configuration** - `gitflic-ci.yaml` ready to use  
✅ **Environment Variables** - 3 format options available  
✅ **Quality Checks** - Formatting, linting, security  
✅ **Intelligent Caching** - Optimized for speed and resource usage  
✅ **Artifact Management** - Automatic cleanup and retention  
✅ **Security Scanning** - Dependency audits and vulnerability checks  

## 📋 Setup Steps

### 1. Upload Variables (Choose One)
Go to `Settings > CI/CD > Variables` and upload:
- `ci-variables.json` (recommended)
- `ci-variables.yaml` 
- `ci-variables.csv`

### 2. Configure GitFlic Settings
- **CI/CD file path**: `gitflic-ci.yaml`
- **Pipeline timeout**: 1 hour (3600 seconds)
- **Auto-cancel redundant pipelines**: ✅ Enable
- **Merge request pipelines**: ✅ Enable
- **Merge trains**: ✅ Enable

### 3. That's It! 
Push to `main` or create a merge request - CI/CD starts automatically.

## 🔄 Pipeline Stages

| Stage | Duration | What It Does |
|-------|----------|--------------|
| 🔧 **Setup** | 2-3 min | Install Rust toolchain, cache dependencies |
| 🔍 **Quality** | 3-5 min | Format check, Clippy linting, compilation |
| 🏗️ **Build** | 5-10 min | Debug + Release builds, examples |
| 🧪 **Test** | 10-15 min | Unit, integration, doc tests |
| 🔒 **Security** | 2-3 min | Dependency audit, vulnerability scan |
| 📚 **Documentation** | 3-5 min | Generate docs, coverage report |
| 🐳 **Docker** | 5-8 min | Build container image, security scan |
| 🚀 **Deploy** | Manual | Artifact publishing, container deployment |

## 📦 Artifacts & Reports

### Automatically Generated
- **Release Builds** (1 week retention)
- **Test Results** (JUnit format)
- **Code Coverage** (Cobertura format)
- **Security Reports** (1 week retention)
- **Documentation** (1 month retention)

### Manual Deployment
- **Final Releases** (3 months retention)
- **Docker Images** (Available in registry)
- **Compliance Reports** (3 months retention)

## 🛠️ Troubleshooting

### Common Issues

**Build timeout?**
```bash
BUILD_TIMEOUT=7200  # Add to variables (2 hours)
```

**Test timeout?**
```bash
TEST_TIMEOUT=3600   # Add to variables (1 hour)
```

**Cache issues?**
```bash
# Clear cache in GitFlic CI/CD settings
# Or add to variables:
CACHE_VERSION=v2
```

**Memory issues?**
```bash
PARALLEL_JOBS=2        # Reduce parallel jobs
CARGO_BUILD_JOBS=2     # Reduce Cargo parallelism
```

## 🎯 Development Workflow

```bash
# Start working
git checkout main
git pull origin main

# Test locally before push
cargo test && cargo clippy && cargo fmt

# Push changes (triggers CI/CD)
git add . && git commit -m "Your changes" && git push

# Create merge request for feature branches
git checkout -b feature/your-feature
# ... make changes ...
git push origin feature/your-feature
# Create MR in GitFlic UI
```

## ⚙️ Key Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_VERSION` | `1.83.0` | Rust toolchain version |
| `COVERAGE_THRESHOLD` | `80` | Minimum code coverage % |
| `BUILD_TIMEOUT` | `3600` | Build timeout in seconds |
| `TEST_TIMEOUT` | `1800` | Test timeout in seconds |
| `NFS_PORT` | `2049` | NFS server port for tests |
| `DOCKER_REGISTRY` | `registry.gitflic.ru` | Docker registry URL |
| `DOCKER_IMAGE_NAME` | `nfs-mamont` | Docker image name |

## 🔐 Security Features

- **Dependency Auditing** - `cargo audit` for known vulnerabilities
- **License Compliance** - Validate open source licenses
- **SAST Analysis** - Static security analysis
- **Secret Detection** - Prevent credential leaks

## 📊 Monitoring

The pipeline provides built-in GitFlic reports:
- **Test Results** - JUnit format with pass/fail tracking
- **Code Coverage** - Cobertura format with trend analysis
- **Security Dashboard** - Vulnerability overview
- **Performance Metrics** - Build time and resource usage

## 🐳 Docker Integration

### Container Registry Setup
1. **Login to registry**:
   ```bash
   docker login registry.gitflic.ru
   # Use your GitFlic username and transport token
   ```

2. **Manual build and push**:
   ```bash
   # Build image
   docker build -t registry.gitflic.ru/project/yadro/nfs-mamont/nfs-mamont:latest .
   
   # Push to registry
   docker push registry.gitflic.ru/project/yadro/nfs-mamont/nfs-mamont:latest
   ```

### Running the Container
```bash
# Run NFS server container
docker run -d \
  --name nfs-mamont \
  -p 2049:2049 \
  --privileged \
  registry.gitflic.ru/project/yadro/nfs-mamont/nfs-mamont:latest

# Mount NFS from container
sudo mount -t nfs -o vers=3,tcp,port=2049 <container-ip>:/ /mnt/nfs
```

### Docker Pipeline Features
- **Multi-stage builds** - Optimized image size
- **Security scanning** - Trivy vulnerability analysis
- **Automatic tagging** - SHA and latest tags
- **Registry integration** - GitFlic container registry
- **Manual deployment** - Controlled releases

## 📚 Additional Resources

- **[GitFlic CI/CD Documentation](https://docs.gitflic.ru/cicd/gitflic-ci-yaml/)** - Official documentation
- **[Rust CI Best Practices](https://doc.rust-lang.org/cargo/guide/continuous-integration.html)** - Rust-specific guidance
- **[Docker Best Practices](https://docs.docker.com/develop/dev-best-practices/)** - Container optimization
- **[Project Issues](../../issues)** - Report problems or ask questions

---

**Ready to code?** Just push to `main` and watch the magic happen! 🎉