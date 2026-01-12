mod api;
mod firecracker;
mod models;
mod state;

use std::net::SocketAddr;
use std::path::Path;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn check_kvm_access() -> Result<(), String> {
    let kvm_path = Path::new("/dev/kvm");

    if !kvm_path.exists() {
        return Err(
            "/dev/kvm not found. KVM may not be enabled.\n\
             To enable KVM:\n  \
             1. Check if your CPU supports virtualization (Intel VT-x or AMD-V)\n  \
             2. Enable virtualization in BIOS/UEFI\n  \
             3. Load the KVM module: sudo modprobe kvm_intel (or kvm_amd)"
                .to_string(),
        );
    }

    // Check read/write access
    match std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(kvm_path)
    {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => Err(
            "/dev/kvm exists but you don't have permission to access it.\n\
             To fix this, add your user to the kvm group:\n  \
             sudo usermod -aG kvm $USER\n\
             Then log out and log back in for the change to take effect."
                .to_string(),
        ),
        Err(e) => Err(format!("Failed to access /dev/kvm: {}", e)),
    }
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "firecracker_control_plane=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Check KVM access before starting
    if let Err(e) = check_kvm_access() {
        tracing::error!("KVM access check failed:\n{}", e);
        std::process::exit(1);
    }
    tracing::info!("KVM access: OK");

    // Create VM manager
    let vm_manager = state::VmManager::new();

    // Create router
    let app = api::create_router(vm_manager).layer(TraceLayer::new_for_http());

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    tracing::info!("Starting Firecracker control plane on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
