mod api;
mod firecracker;
mod models;
mod persistence;
mod state;

use std::io::{self, Write};
use std::net::SocketAddr;
use std::path::Path;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn print_status(msg: &str) {
    print!("  {}... ", msg);
    let _ = io::stdout().flush();
}

fn print_banner() {
    let db_path = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".glidex")
        .join("glidex.db");

    println!();
    println!("  ╔═══════════════════════════════════════════╗");
    println!("  ║            GlideX Control Plane           ║");
    println!("  ╚═══════════════════════════════════════════╝");
    println!();
    println!("  Version:   {}", VERSION);
    println!("  Database:  {}", db_path.display());
    println!();
}

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
    // Print startup banner
    print_banner();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "glidex_control_plane=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Check KVM access before starting
    print_status("Checking KVM access");
    if let Err(e) = check_kvm_access() {
        println!("FAILED");
        eprintln!("\n{}", e);
        std::process::exit(1);
    }
    println!("OK");

    // Create VM manager with persistence
    print_status("Opening database");
    let vm_manager = match state::VmManager::new() {
        Ok(manager) => manager,
        Err(e) => {
            println!("FAILED");
            eprintln!("\nFailed to initialize VM manager: {}", e);
            std::process::exit(1);
        }
    };
    println!("OK");

    // Initialize: load persisted VMs and reconcile state
    print_status("Loading VMs");
    if let Err(e) = vm_manager.initialize().await {
        println!("FAILED");
        eprintln!("\nFailed to initialize VMs from database: {}", e);
        std::process::exit(1);
    }
    let vm_count = vm_manager.list_vms().await.len();
    println!("OK ({} VMs)", vm_count);

    // Create router
    let app = api::create_router(vm_manager).layer(TraceLayer::new_for_http());

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    println!();
    println!("  Listening on http://{}", addr);
    println!();

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
