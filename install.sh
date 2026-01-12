#!/bin/bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Versions
FIRECRACKER_VERSION="v1.14.0"
RUST_MIN_VERSION="1.85.0"

echo -e "${CYAN}"
echo "   _____ _ _     _"
echo "  / ____| (_)   | |"
echo " | |  __| |_  __| | _____  __"
echo " | | |_ | | |/ _\` |/ _ \\ \\/ /"
echo " | |__| | | | (_| |  __/>  <"
echo "  \\_____|_|_|\\__,_|\\___/_/\\_\\"
echo "        Control Plane Installer"
echo -e "${NC}"
echo ""

# Detect OS and architecture
detect_platform() {
    OS=$(uname -s | tr '[:upper:]' '[:lower:]')
    ARCH=$(uname -m)

    case "$ARCH" in
        x86_64|amd64)
            ARCH="x86_64"
            ;;
        aarch64|arm64)
            ARCH="aarch64"
            ;;
        *)
            echo -e "${RED}Error: Unsupported architecture: $ARCH${NC}"
            exit 1
            ;;
    esac

    if [ "$OS" != "linux" ]; then
        echo -e "${RED}Error: Firecracker only supports Linux${NC}"
        exit 1
    fi

    echo -e "${GREEN}Detected platform: ${OS}-${ARCH}${NC}"
}

# Check if running as root for system-wide installation
check_privileges() {
    if [ "$EUID" -eq 0 ]; then
        INSTALL_DIR="/usr/local/bin"
        echo -e "${YELLOW}Running as root, will install to ${INSTALL_DIR}${NC}"
    else
        INSTALL_DIR="$HOME/.local/bin"
        mkdir -p "$INSTALL_DIR"
        echo -e "${YELLOW}Running as user, will install to ${INSTALL_DIR}${NC}"
        echo -e "${YELLOW}Make sure ${INSTALL_DIR} is in your PATH${NC}"
    fi
}

# Check if a command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Install Rust
install_rust() {
    echo ""
    echo -e "${CYAN}=== Checking Rust ===${NC}"

    if command_exists rustc; then
        RUST_VERSION=$(rustc --version | awk '{print $2}')
        echo -e "${GREEN}Rust is already installed: ${RUST_VERSION}${NC}"

        # Check if version is sufficient
        if [ "$(printf '%s\n' "$RUST_MIN_VERSION" "$RUST_VERSION" | sort -V | head -n1)" = "$RUST_MIN_VERSION" ]; then
            echo -e "${GREEN}Rust version is sufficient${NC}"
            return 0
        else
            echo -e "${YELLOW}Rust version is older than ${RUST_MIN_VERSION}, updating...${NC}"
        fi
    else
        echo -e "${YELLOW}Rust is not installed, installing...${NC}"
    fi

    # Install or update Rust using rustup
    if command_exists rustup; then
        echo "Updating Rust via rustup..."
        rustup update stable
    else
        echo "Installing Rust via rustup..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
        source "$HOME/.cargo/env"
    fi

    echo -e "${GREEN}Rust installed successfully: $(rustc --version)${NC}"
}

# Install Firecracker
install_firecracker() {
    echo ""
    echo -e "${CYAN}=== Checking Firecracker ===${NC}"

    if command_exists firecracker; then
        FC_VERSION=$(firecracker --version 2>&1 | head -n1)
        echo -e "${GREEN}Firecracker is already installed: ${FC_VERSION}${NC}"

        read -p "Do you want to reinstall/update Firecracker? [y/N] " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            return 0
        fi
    fi

    echo -e "${YELLOW}Installing Firecracker ${FIRECRACKER_VERSION}...${NC}"

    # Create temp directory
    TEMP_DIR=$(mktemp -d)
    cd "$TEMP_DIR"

    # Download Firecracker release
    RELEASE_URL="https://github.com/firecracker-microvm/firecracker/releases/download/${FIRECRACKER_VERSION}/firecracker-${FIRECRACKER_VERSION}-${ARCH}.tgz"

    echo "Downloading from: ${RELEASE_URL}"
    curl -sSL "$RELEASE_URL" -o firecracker.tgz

    # Extract
    tar -xzf firecracker.tgz

    # Find and install binaries
    RELEASE_DIR="release-${FIRECRACKER_VERSION}-${ARCH}"

    if [ -d "$RELEASE_DIR" ]; then
        # Install firecracker
        if [ -f "${RELEASE_DIR}/firecracker-${FIRECRACKER_VERSION}-${ARCH}" ]; then
            sudo install -o root -g root -m 0755 "${RELEASE_DIR}/firecracker-${FIRECRACKER_VERSION}-${ARCH}" "${INSTALL_DIR}/firecracker"
            echo -e "${GREEN}Installed firecracker to ${INSTALL_DIR}/firecracker${NC}"
        fi

        # Install jailer (optional but useful)
        if [ -f "${RELEASE_DIR}/jailer-${FIRECRACKER_VERSION}-${ARCH}" ]; then
            sudo install -o root -g root -m 0755 "${RELEASE_DIR}/jailer-${FIRECRACKER_VERSION}-${ARCH}" "${INSTALL_DIR}/jailer"
            echo -e "${GREEN}Installed jailer to ${INSTALL_DIR}/jailer${NC}"
        fi
    else
        echo -e "${RED}Error: Could not find release directory${NC}"
        ls -la
        exit 1
    fi

    # Cleanup
    cd -
    rm -rf "$TEMP_DIR"

    echo -e "${GREEN}Firecracker installed successfully: $(firecracker --version 2>&1 | head -n1)${NC}"
}

# Download sample kernel and rootfs
download_samples() {
    echo ""
    echo -e "${CYAN}=== Sample Kernel and RootFS ===${NC}"

    read -p "Do you want to download sample kernel and rootfs? [y/N] " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        return 0
    fi

    SAMPLE_DIR="$HOME/.glidex"
    mkdir -p "$SAMPLE_DIR"
    cd "$SAMPLE_DIR"

    # Check for required tools
    if ! command_exists unsquashfs; then
        echo -e "${YELLOW}Installing squashfs-tools for rootfs extraction...${NC}"
        if command_exists apt-get; then
            sudo apt-get update && sudo apt-get install -y squashfs-tools
        elif command_exists dnf; then
            sudo dnf install -y squashfs-tools
        elif command_exists yum; then
            sudo yum install -y squashfs-tools
        else
            echo -e "${RED}Error: Please install squashfs-tools manually${NC}"
            return 1
        fi
    fi

    # Get latest Firecracker CI version for image URLs
    echo "Discovering latest kernel and rootfs versions..."
    release_url="https://github.com/firecracker-microvm/firecracker/releases"
    latest_version=$(basename $(curl -fsSLI -o /dev/null -w %{url_effective} ${release_url}/latest))
    CI_VERSION=${latest_version%.*}

    echo "Using Firecracker CI version: $CI_VERSION"

    # Download kernel
    if [ ! -f "$SAMPLE_DIR/vmlinux.bin" ]; then
        echo "Finding latest kernel..."
        latest_kernel_key=$(curl -s "http://spec.ccfc.min.s3.amazonaws.com/?prefix=firecracker-ci/$CI_VERSION/$ARCH/vmlinux-&list-type=2" \
            | grep -oP "(?<=<Key>)(firecracker-ci/$CI_VERSION/$ARCH/vmlinux-[0-9]+\.[0-9]+\.[0-9]{1,3})(?=</Key>)" \
            | sort -V | tail -1)

        if [ -z "$latest_kernel_key" ]; then
            echo -e "${RED}Error: Could not find kernel image${NC}"
            return 1
        fi

        echo "Downloading kernel: $latest_kernel_key"
        curl -sSL "https://s3.amazonaws.com/spec.ccfc.min/${latest_kernel_key}" -o "$SAMPLE_DIR/vmlinux.bin"
        echo -e "${GREEN}Downloaded kernel to $SAMPLE_DIR/vmlinux.bin${NC}"
    else
        echo -e "${GREEN}Kernel already exists at $SAMPLE_DIR/vmlinux.bin${NC}"
    fi

    # Download and convert rootfs
    if [ ! -f "$SAMPLE_DIR/rootfs.ext4" ]; then
        echo "Finding latest Ubuntu rootfs..."
        latest_ubuntu_key=$(curl -s "http://spec.ccfc.min.s3.amazonaws.com/?prefix=firecracker-ci/$CI_VERSION/$ARCH/ubuntu-&list-type=2" \
            | grep -oP "(?<=<Key>)(firecracker-ci/$CI_VERSION/$ARCH/ubuntu-[0-9]+\.[0-9]+\.squashfs)(?=</Key>)" \
            | sort -V | tail -1)

        if [ -z "$latest_ubuntu_key" ]; then
            echo -e "${RED}Error: Could not find rootfs image${NC}"
            return 1
        fi

        ubuntu_version=$(basename $latest_ubuntu_key .squashfs | grep -oE '[0-9]+\.[0-9]+')
        echo "Downloading Ubuntu $ubuntu_version rootfs: $latest_ubuntu_key"

        # Download squashfs
        curl -sSL "https://s3.amazonaws.com/spec.ccfc.min/$latest_ubuntu_key" -o "ubuntu.squashfs"

        echo "Converting squashfs to ext4 (this may take a moment)..."

        # Extract squashfs
        sudo unsquashfs -d squashfs-root ubuntu.squashfs

        # Generate SSH key for VM access
        if [ ! -f "$SAMPLE_DIR/vm_key" ]; then
            ssh-keygen -f "$SAMPLE_DIR/vm_key" -N "" -q
            echo -e "${GREEN}Generated SSH key: $SAMPLE_DIR/vm_key${NC}"
        fi

        # Add SSH key to rootfs
        sudo mkdir -p squashfs-root/root/.ssh
        sudo cp "$SAMPLE_DIR/vm_key.pub" squashfs-root/root/.ssh/authorized_keys
        sudo chmod 600 squashfs-root/root/.ssh/authorized_keys

        # Create ext4 filesystem
        truncate -s 1G "$SAMPLE_DIR/rootfs.ext4"
        sudo mkfs.ext4 -d squashfs-root -F "$SAMPLE_DIR/rootfs.ext4" >/dev/null 2>&1

        # Cleanup
        sudo rm -rf squashfs-root ubuntu.squashfs

        echo -e "${GREEN}Downloaded and converted rootfs to $SAMPLE_DIR/rootfs.ext4${NC}"
    else
        echo -e "${GREEN}RootFS already exists at $SAMPLE_DIR/rootfs.ext4${NC}"
    fi

    cd - > /dev/null

    echo ""
    echo -e "${CYAN}Sample files location:${NC}"
    echo "  Kernel:  $SAMPLE_DIR/vmlinux.bin"
    echo "  RootFS:  $SAMPLE_DIR/rootfs.ext4"
    if [ -f "$SAMPLE_DIR/vm_key" ]; then
        echo "  SSH Key: $SAMPLE_DIR/vm_key"
        echo ""
        echo -e "${YELLOW}To SSH into a running VM:${NC}"
        echo "  ssh -i $SAMPLE_DIR/vm_key root@<vm-ip>"
    fi
}

# Build the project
build_project() {
    echo ""
    echo -e "${CYAN}=== Building Firecracker Control Plane ===${NC}"

    # Make sure we're in the project directory
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    cd "$SCRIPT_DIR"

    if [ ! -f "Cargo.toml" ]; then
        echo -e "${RED}Error: Cargo.toml not found. Are you in the project directory?${NC}"
        exit 1
    fi

    # Source cargo env if needed
    if [ -f "$HOME/.cargo/env" ]; then
        source "$HOME/.cargo/env"
    fi

    echo "Building project..."
    cargo build --release

    echo -e "${GREEN}Build successful!${NC}"
    echo ""
    echo -e "${CYAN}Binaries location:${NC}"
    echo "  Server: target/release/glidex-control-plane"
    echo "  CLI:    target/release/gxctl"

    read -p "Do you want to install binaries to ${INSTALL_DIR}? [y/N] " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        if [ "$EUID" -eq 0 ]; then
            install -m 0755 target/release/glidex-control-plane "${INSTALL_DIR}/"
            install -m 0755 target/release/gxctl "${INSTALL_DIR}/"
        else
            install -m 0755 target/release/glidex-control-plane "${INSTALL_DIR}/"
            install -m 0755 target/release/gxctl "${INSTALL_DIR}/"
        fi
        echo -e "${GREEN}Installed binaries to ${INSTALL_DIR}${NC}"
    fi
}

# Setup KVM permissions
setup_kvm() {
    echo ""
    echo -e "${CYAN}=== Checking KVM Access ===${NC}"

    if [ ! -e /dev/kvm ]; then
        echo -e "${RED}Warning: /dev/kvm not found. KVM may not be enabled.${NC}"
        echo "To enable KVM:"
        echo "  1. Check if your CPU supports virtualization (Intel VT-x or AMD-V)"
        echo "  2. Enable virtualization in BIOS/UEFI"
        echo "  3. Load the KVM module: sudo modprobe kvm_intel (or kvm_amd)"
        return 1
    fi

    if [ -r /dev/kvm ] && [ -w /dev/kvm ]; then
        echo -e "${GREEN}KVM access: OK${NC}"
    else
        echo -e "${YELLOW}KVM device exists but you don't have access${NC}"
        echo "Adding user to kvm group..."

        if [ "$EUID" -eq 0 ]; then
            usermod -aG kvm "$SUDO_USER"
            echo -e "${GREEN}Added $SUDO_USER to kvm group${NC}"
            echo -e "${YELLOW}Please log out and log back in for changes to take effect${NC}"
        else
            echo "Run: sudo usermod -aG kvm $USER"
            echo "Then log out and log back in"
        fi
    fi
}

# Print usage instructions
print_usage() {
    echo ""
    echo -e "${CYAN}=== Quick Start ===${NC}"
    echo ""
    echo "1. Start the server:"
    echo -e "   ${GREEN}glidex-control-plane${NC}"
    echo ""
    echo "2. In another terminal, start the CLI:"
    echo -e "   ${GREEN}gxctl${NC}"
    echo ""
    echo "3. Create and start a VM:"
    echo -e "   ${GREEN}gxctl> create${NC}"
    echo "   Enter VM details (or press Enter for defaults)"
    echo -e "   ${GREEN}gxctl> start <vm-name>${NC}"
    echo ""
    echo "4. Connect to VM console:"
    echo -e "   ${GREEN}gxctl> connect <vm-name>${NC}"
    echo "   (Press Ctrl+] to detach)"
    echo ""
    echo "5. View console log:"
    echo -e "   ${GREEN}gxctl> log <vm-name>${NC}"
    echo ""
    echo "For more commands, type 'help' in gxctl"
    echo ""
}

# Main installation flow
main() {
    detect_platform
    check_privileges

    echo ""
    echo -e "${CYAN}This script will install:${NC}"
    echo "  1. Rust (if not installed)"
    echo "  2. Firecracker ${FIRECRACKER_VERSION}"
    echo "  3. Build the control plane"
    echo "  4. (Optional) Download sample kernel and rootfs"
    echo ""

    read -p "Continue with installation? [Y/n] " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Nn]$ ]]; then
        echo "Installation cancelled"
        exit 0
    fi

    install_rust
    install_firecracker
    setup_kvm
    build_project
    download_samples
    print_usage

    echo -e "${GREEN}Installation complete!${NC}"
}

# Run main function
main "$@"
