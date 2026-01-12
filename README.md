# Glidex

A Rust-based control plane for managing [Firecracker](https://firecracker-microvm.github.io/) microVMs.

```
   _____ _ _     _
  / ____| (_)   | |
 | |  __| |_  __| | _____  __
 | | |_ | | |/ _` |/ _ \ \/ /
 | |__| | | | (_| |  __/>  <
  \_____|_|_|\__,_|\___/_/\_\
```

## Features

- **REST API** for VM lifecycle management (create, start, stop, pause, delete)
- **Interactive CLI** (`gxctl`) with command history and tab completion
- **Interactive console** - connect to VM serial console with full I/O support
- **Console logging** - persistent logs of all VM console output
- **Multi-client support** - multiple CLI sessions can connect to the same VM console

## Quick Start

### Installation

```bash
# Clone the repository
git clone https://github.com/yourusername/glidex.git
cd glidex

# Run the installer (installs Rust, Firecracker, builds the project)
./install.sh
```

The installer will:
1. Install Rust (if not present)
2. Download and install Firecracker v1.14.0
3. Build the Glidex binaries
4. Optionally download sample kernel and rootfs images to `~/.glidex/`

### Manual Installation

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build the project
cargo build --release

# Binaries are in target/release/
#   - glidex-control-plane (server)
#   - gxctl (CLI)
```

### Running

1. **Start the control plane server:**

```bash
glidex-control-plane
```

The server listens on `http://localhost:8080` by default.

2. **In another terminal, start the CLI:**

```bash
gxctl
```

3. **Create and start a VM:**

```
gxctl> create
VM name: my-vm
vCPU count [1]: 2
Memory (MiB) [512]: 1024
Kernel image path [~/.glidex/vmlinux.bin]:
Root filesystem path [~/.glidex/rootfs.ext4]:
Kernel arguments (optional):

gxctl> start my-vm
```

4. **Connect to the VM console:**

```
gxctl> connect my-vm
```

Press `Ctrl+]` to detach from the console.

5. **View console logs:**

```
gxctl> log my-vm
```

## CLI Commands

| Command | Description |
|---------|-------------|
| `list` | List all VMs |
| `get <name\|id>` | Show VM details |
| `create` | Create a new VM (interactive) |
| `start <name\|id>` | Start a VM |
| `stop <name\|id>` | Stop a VM |
| `pause <name\|id>` | Pause a running VM |
| `connect <name\|id>` | Connect to VM console (interactive) |
| `log <name\|id>` | Show VM serial console log |
| `delete <name\|id>` | Delete a VM |
| `health` | Check API server health |
| `help` | Show available commands |
| `exit` | Exit the CLI |

## REST API

### Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/health` | Health check |
| `GET` | `/vms` | List all VMs |
| `POST` | `/vms` | Create a new VM |
| `GET` | `/vms/{id}` | Get VM details |
| `DELETE` | `/vms/{id}` | Delete a VM |
| `POST` | `/vms/{id}/start` | Start a VM |
| `POST` | `/vms/{id}/stop` | Stop a VM |
| `POST` | `/vms/{id}/pause` | Pause a VM |
| `GET` | `/vms/{id}/console` | Get console connection info |

### Create VM Request

```json
{
  "name": "my-vm",
  "vcpu_count": 2,
  "mem_size_mib": 1024,
  "kernel_image_path": "/path/to/vmlinux.bin",
  "rootfs_path": "/path/to/rootfs.ext4",
  "kernel_args": "console=ttyS0 reboot=k panic=1 pci=off"
}
```

### Example: Create and Start a VM with curl

```bash
# Create a VM
curl -X POST http://localhost:8080/vms \
  -H "Content-Type: application/json" \
  -d '{
    "name": "test-vm",
    "vcpu_count": 2,
    "mem_size_mib": 512,
    "kernel_image_path": "/home/user/.glidex/vmlinux.bin",
    "rootfs_path": "/home/user/.glidex/rootfs.ext4"
  }'

# Start the VM
curl -X POST http://localhost:8080/vms/{vm-id}/start

# List VMs
curl http://localhost:8080/vms
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      gxctl (CLI)                            │
│  - Interactive shell                                        │
│  - Connects to REST API                                     │
│  - Connects to console Unix socket                          │
└─────────────────┬───────────────────────────────────────────┘
                  │ HTTP / Unix Socket
┌─────────────────▼───────────────────────────────────────────┐
│              glidex-control-plane (Server)                  │
│  - REST API (Axum)                                          │
│  - VM state management                                      │
│  - Console proxy (PTY ↔ Unix socket)                        │
│  - Console logging                                          │
└─────────────────┬───────────────────────────────────────────┘
                  │ Unix Socket (Firecracker API)
┌─────────────────▼───────────────────────────────────────────┐
│                    Firecracker                              │
│  - microVM hypervisor                                       │
│  - KVM-based virtualization                                 │
└─────────────────────────────────────────────────────────────┘
```

### Console Architecture

For each running VM:
- A PTY (pseudo-terminal) pair is created
- Firecracker's stdin/stdout/stderr connect to the PTY slave
- A background thread reads from the PTY master and:
  - Writes all output to a log file (`/tmp/firecracker-{id}.log`)
  - Broadcasts to connected clients via Unix socket (`/tmp/firecracker-{id}.console.sock`)
- Multiple clients can connect simultaneously

## Requirements

- **Linux** (Firecracker only supports Linux)
- **KVM** enabled (`/dev/kvm` accessible)
- **Rust 1.85+** (for building)
- **Firecracker 1.14.0+**

### Enabling KVM

```bash
# Check if KVM is available
ls -la /dev/kvm

# Add your user to the kvm group
sudo usermod -aG kvm $USER

# Log out and back in for the change to take effect
```

## Project Structure

```
glidex/
├── src/
│   ├── main.rs          # Server entry point
│   ├── lib.rs           # Library exports
│   ├── api.rs           # REST API routes and handlers
│   ├── models.rs        # Data structures (VM, VmConfig, etc.)
│   ├── state.rs         # VM state management
│   ├── firecracker.rs   # Firecracker API client & process management
│   └── bin/
│       └── gxctl.rs     # CLI client
├── tests/
│   └── api_tests.rs     # API integration tests
├── install.sh           # Installation script
├── Cargo.toml           # Rust dependencies
└── README.md
```

## Development

### Running Tests

```bash
cargo test
```

### Building in Debug Mode

```bash
cargo build
```

### Building for Release

```bash
cargo build --release
```

## Sample Kernel and RootFS

The installer can download official Firecracker CI images:

- **Kernel**: Linux kernel compiled for Firecracker
- **RootFS**: Ubuntu-based root filesystem (ext4)
- **SSH Key**: Generated key pair for VM access

Files are stored in `~/.glidex/`:
```
~/.glidex/
├── vmlinux.bin    # Linux kernel
├── rootfs.ext4    # Ubuntu root filesystem
├── vm_key         # SSH private key
└── vm_key.pub     # SSH public key
```

To SSH into a running VM (requires network configuration):
```bash
ssh -i ~/.glidex/vm_key root@<vm-ip>
```

## License

MIT

## Acknowledgments

- [Firecracker](https://firecracker-microvm.github.io/) - The microVM hypervisor
- [Axum](https://github.com/tokio-rs/axum) - Web framework
- [Tokio](https://tokio.rs/) - Async runtime
