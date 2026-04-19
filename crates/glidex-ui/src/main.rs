use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;
use tokio::signal;

fn ui_dir() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir).join("ui")
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let ui_path = ui_dir();
    tracing::info!("Starting UI dev server from {}", ui_path.display());

    let mut child = Command::new("bun")
        .arg("run")
        .arg("dev")
        .current_dir(&ui_path)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("Failed to start bun dev server. Is bun installed?");

    tracing::info!("GlideX UI available at http://localhost:5173");

    tokio::select! {
        status = child.wait() => {
            match status {
                Ok(s) => tracing::info!("Bun dev server exited with {}", s),
                Err(e) => tracing::error!("Bun dev server error: {}", e),
            }
        }
        _ = signal::ctrl_c() => {
            tracing::info!("Shutting down...");
            let _ = child.kill().await;
        }
    }
}
