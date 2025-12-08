use crate::models::{Vm, VmConfig};
use nix::pty::{openpty, OpenptyResult};
use nix::unistd::setsid;
use serde::Serialize;
use std::fs::{File, OpenOptions};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::CommandExt;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use std::thread;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FirecrackerError {
    #[error("Failed to start Firecracker process: {0}")]
    ProcessStart(#[from] std::io::Error),
    #[error("Failed to connect to Firecracker socket: {0}")]
    SocketConnection(String),
    #[error("API request failed: {0}")]
    ApiRequest(String),
}

#[derive(Debug, Serialize)]
struct BootSource {
    kernel_image_path: String,
    boot_args: String,
}

#[derive(Debug, Serialize)]
struct MachineConfig {
    vcpu_count: u8,
    mem_size_mib: u32,
}

#[derive(Debug, Serialize)]
struct Drive {
    drive_id: String,
    path_on_host: String,
    is_root_device: bool,
    is_read_only: bool,
}

#[derive(Debug, Serialize)]
struct InstanceAction {
    action_type: String,
}

pub struct FirecrackerClient {
    socket_path: String,
}

impl FirecrackerClient {
    pub fn new(socket_path: &str) -> Self {
        Self {
            socket_path: socket_path.to_string(),
        }
    }

    fn send_request(&self, method: &str, path: &str, body: Option<&str>) -> Result<String, FirecrackerError> {
        let stream = UnixStream::connect(&self.socket_path)
            .map_err(|e| FirecrackerError::SocketConnection(e.to_string()))?;

        // Set read timeout to prevent hanging
        stream.set_read_timeout(Some(Duration::from_secs(30)))?;

        let mut writer = stream.try_clone()?;
        let mut reader = BufReader::new(stream);

        let body_str = body.unwrap_or("");
        let content_length = body_str.len();

        let request = format!(
            "{} {} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            method, path, content_length, body_str
        );

        writer.write_all(request.as_bytes())?;
        writer.flush()?;

        // Read HTTP response headers
        let mut response = String::new();
        let mut content_length: usize = 0;

        loop {
            let mut line = String::new();
            reader.read_line(&mut line)?;
            response.push_str(&line);

            // Check for Content-Length header
            if line.to_lowercase().starts_with("content-length:") {
                if let Some(len_str) = line.split(':').nth(1) {
                    content_length = len_str.trim().parse().unwrap_or(0);
                }
            }

            // Empty line marks end of headers
            if line == "\r\n" || line == "\n" {
                break;
            }
        }

        // Read body if there is one
        if content_length > 0 {
            let mut body_buf = vec![0u8; content_length];
            reader.read_exact(&mut body_buf)?;
            response.push_str(&String::from_utf8_lossy(&body_buf));
        }

        Ok(response)
    }

    pub fn configure_machine(&self, config: &VmConfig) -> Result<(), FirecrackerError> {
        let machine_config = MachineConfig {
            vcpu_count: config.vcpu_count,
            mem_size_mib: config.mem_size_mib,
        };

        let body = serde_json::to_string(&machine_config)
            .map_err(|e| FirecrackerError::ApiRequest(e.to_string()))?;

        let response = self.send_request("PUT", "/machine-config", Some(&body))?;

        if !response.contains("HTTP/1.1 204") && !response.contains("HTTP/1.1 200") {
            return Err(FirecrackerError::ApiRequest(format!(
                "Failed to configure machine: {}",
                response
            )));
        }

        Ok(())
    }

    pub fn set_boot_source(&self, config: &VmConfig) -> Result<(), FirecrackerError> {
        let boot_source = BootSource {
            kernel_image_path: config.kernel_image_path.clone(),
            boot_args: config.kernel_args.clone(),
        };

        let body = serde_json::to_string(&boot_source)
            .map_err(|e| FirecrackerError::ApiRequest(e.to_string()))?;

        let response = self.send_request("PUT", "/boot-source", Some(&body))?;

        if !response.contains("HTTP/1.1 204") && !response.contains("HTTP/1.1 200") {
            return Err(FirecrackerError::ApiRequest(format!(
                "Failed to set boot source: {}",
                response
            )));
        }

        Ok(())
    }

    pub fn add_root_drive(&self, rootfs_path: &str) -> Result<(), FirecrackerError> {
        let drive = Drive {
            drive_id: "rootfs".to_string(),
            path_on_host: rootfs_path.to_string(),
            is_root_device: true,
            is_read_only: false,
        };

        let body = serde_json::to_string(&drive)
            .map_err(|e| FirecrackerError::ApiRequest(e.to_string()))?;

        let response = self.send_request("PUT", "/drives/rootfs", Some(&body))?;

        if !response.contains("HTTP/1.1 204") && !response.contains("HTTP/1.1 200") {
            return Err(FirecrackerError::ApiRequest(format!(
                "Failed to add root drive: {}",
                response
            )));
        }

        Ok(())
    }

    pub fn start_instance(&self) -> Result<(), FirecrackerError> {
        let action = InstanceAction {
            action_type: "InstanceStart".to_string(),
        };

        let body = serde_json::to_string(&action)
            .map_err(|e| FirecrackerError::ApiRequest(e.to_string()))?;

        let response = self.send_request("PUT", "/actions", Some(&body))?;

        if !response.contains("HTTP/1.1 204") && !response.contains("HTTP/1.1 200") {
            return Err(FirecrackerError::ApiRequest(format!(
                "Failed to start instance: {}",
                response
            )));
        }

        Ok(())
    }

    pub fn pause_instance(&self) -> Result<(), FirecrackerError> {
        let body = r#"{"state": "Paused"}"#;
        let response = self.send_request("PATCH", "/vm", Some(body))?;

        if !response.contains("HTTP/1.1 204") && !response.contains("HTTP/1.1 200") {
            return Err(FirecrackerError::ApiRequest(format!(
                "Failed to pause instance: {}",
                response
            )));
        }

        Ok(())
    }

    pub fn resume_instance(&self) -> Result<(), FirecrackerError> {
        let body = r#"{"state": "Resumed"}"#;
        let response = self.send_request("PATCH", "/vm", Some(body))?;

        if !response.contains("HTTP/1.1 204") && !response.contains("HTTP/1.1 200") {
            return Err(FirecrackerError::ApiRequest(format!(
                "Failed to resume instance: {}",
                response
            )));
        }

        Ok(())
    }
}

pub struct FirecrackerProcess {
    pub child: Child,
    pub socket_path: String,
    pub console_socket_path: String,
    #[allow(dead_code)]
    pub log_path: String,
    running: Arc<AtomicBool>,
    console_thread: Option<thread::JoinHandle<()>>,
}

impl FirecrackerProcess {
    pub fn spawn(socket_path: &str, console_socket_path: &str, log_path: &str) -> Result<Self, FirecrackerError> {
        // Remove existing sockets if present
        let _ = std::fs::remove_file(socket_path);
        let _ = std::fs::remove_file(console_socket_path);

        // Create/truncate log file
        let log_file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(log_path)?;

        // Create a PTY pair for interactive console
        let OpenptyResult { master, slave } = openpty(None, None)
            .map_err(|e| FirecrackerError::SocketConnection(format!("Failed to create PTY: {}", e)))?;

        // Convert slave fd to Stdio for the child process
        let slave_raw = slave.as_raw_fd();
        let stdin_fd = unsafe { File::from_raw_fd(libc::dup(slave_raw)) };
        let stdout_fd = unsafe { File::from_raw_fd(libc::dup(slave_raw)) };
        let stderr_fd = unsafe { File::from_raw_fd(libc::dup(slave_raw)) };

        // Spawn firecracker with the PTY as stdin/stdout/stderr
        let child = unsafe {
            Command::new("firecracker")
                .arg("--api-sock")
                .arg(socket_path)
                .stdin(Stdio::from(stdin_fd))
                .stdout(Stdio::from(stdout_fd))
                .stderr(Stdio::from(stderr_fd))
                .pre_exec(|| {
                    // Create a new session and set controlling terminal
                    setsid().ok();
                    Ok(())
                })
                .spawn()?
        };

        // Drop slave fd - we only need the master
        drop(slave);

        // Create Unix socket for console connections
        let console_listener = UnixListener::bind(console_socket_path)
            .map_err(|e| FirecrackerError::SocketConnection(format!("Failed to create console socket: {}", e)))?;
        console_listener.set_nonblocking(true)
            .map_err(|e| FirecrackerError::SocketConnection(format!("Failed to set non-blocking: {}", e)))?;

        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();
        let log_path_clone = log_path.to_string();

        // Spawn thread to handle console I/O
        let console_thread = thread::spawn(move || {
            Self::console_proxy_loop(master, console_listener, log_file, &log_path_clone, running_clone);
        });

        // Wait for API socket to be available
        for _ in 0..50 {
            if std::path::Path::new(socket_path).exists() {
                return Ok(Self {
                    child,
                    socket_path: socket_path.to_string(),
                    console_socket_path: console_socket_path.to_string(),
                    log_path: log_path.to_string(),
                    running,
                    console_thread: Some(console_thread),
                });
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        Err(FirecrackerError::SocketConnection(
            "Socket not available after timeout".to_string(),
        ))
    }

    fn console_proxy_loop(
        master: OwnedFd,
        listener: UnixListener,
        mut log_file: File,
        log_path: &str,
        running: Arc<AtomicBool>,
    ) {
        let master_raw = master.as_raw_fd();
        let mut clients: Vec<UnixStream> = Vec::new();
        let mut buf = [0u8; 4096];

        // Set master to non-blocking
        unsafe {
            let flags = libc::fcntl(master_raw, libc::F_GETFL);
            libc::fcntl(master_raw, libc::F_SETFL, flags | libc::O_NONBLOCK);
        }

        while running.load(Ordering::SeqCst) {
            // Accept new client connections
            if let Ok((stream, _)) = listener.accept() {
                stream.set_nonblocking(true).ok();
                // Send existing log content to new client
                if let Ok(mut existing_log) = File::open(log_path) {
                    let mut log_content = Vec::new();
                    if existing_log.read_to_end(&mut log_content).is_ok() && !log_content.is_empty() {
                        let mut s = stream.try_clone().unwrap();
                        let _ = s.write_all(&log_content);
                    }
                }
                clients.push(stream);
            }

            // Read from PTY master and broadcast to clients + log file
            let master_file = unsafe { File::from_raw_fd(libc::dup(master_raw)) };
            let mut master_reader = master_file;
            match master_reader.read(&mut buf) {
                Ok(0) => break, // PTY closed
                Ok(n) => {
                    let data = &buf[..n];

                    // Write to log file
                    let _ = log_file.write_all(data);
                    let _ = log_file.flush();

                    // Broadcast to all connected clients
                    clients.retain_mut(|client| {
                        client.write_all(data).is_ok()
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(_) => break,
            }

            // Read from clients and write to PTY master
            for client in &mut clients {
                match client.read(&mut buf) {
                    Ok(0) => {} // Will be cleaned up later
                    Ok(n) => {
                        let mut master_writer = unsafe { File::from_raw_fd(libc::dup(master_raw)) };
                        let _ = master_writer.write_all(&buf[..n]);
                        let _ = master_writer.flush();
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(_) => {}
                }
            }

            thread::sleep(Duration::from_millis(10));
        }
    }

    pub fn kill(&mut self) -> Result<(), FirecrackerError> {
        self.running.store(false, Ordering::SeqCst);
        self.child.kill()?;

        // Wait for console thread to finish
        if let Some(handle) = self.console_thread.take() {
            let _ = handle.join();
        }

        let _ = std::fs::remove_file(&self.socket_path);
        let _ = std::fs::remove_file(&self.console_socket_path);
        Ok(())
    }
}

pub fn configure_vm(vm: &Vm) -> Result<(), FirecrackerError> {
    let client = FirecrackerClient::new(&vm.socket_path);

    // Configure machine
    client.configure_machine(&vm.config)?;

    // Set boot source
    client.set_boot_source(&vm.config)?;

    // Add root drive
    client.add_root_drive(&vm.config.rootfs_path)?;

    Ok(())
}

pub fn start_vm(vm: &Vm) -> Result<(), FirecrackerError> {
    let client = FirecrackerClient::new(&vm.socket_path);
    client.start_instance()
}

pub fn pause_vm(vm: &Vm) -> Result<(), FirecrackerError> {
    let client = FirecrackerClient::new(&vm.socket_path);
    client.pause_instance()
}

pub fn resume_vm(vm: &Vm) -> Result<(), FirecrackerError> {
    let client = FirecrackerClient::new(&vm.socket_path);
    client.resume_instance()
}
