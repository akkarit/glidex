use super::{Hypervisor, HypervisorError, HypervisorProcess, HypervisorType};
use crate::models::VmConfig;
use serde::Serialize;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Cloud-Hypervisor API request structures
#[derive(Debug, Serialize)]
struct CpuConfig {
    boot_vcpus: u8,
    max_vcpus: u8,
}

#[derive(Debug, Serialize)]
struct MemoryConfig {
    size: u64, // bytes
}

#[derive(Debug, Serialize)]
struct PayloadConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    firmware: Option<String>,
    kernel: String,
    cmdline: String,
}

#[derive(Debug, Serialize)]
struct DiskConfig {
    path: String,
}

#[derive(Debug, Serialize)]
struct ConsoleConfig {
    mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
}

#[derive(Debug, Serialize)]
struct VmCreateConfig {
    cpus: CpuConfig,
    memory: MemoryConfig,
    payload: PayloadConfig,
    disks: Vec<DiskConfig>,
    console: ConsoleConfig,
    serial: ConsoleConfig,
}

/// Find the end of HTTP headers (position after the \r\n\r\n separator).
fn find_header_end(data: &[u8]) -> Option<usize> {
    data.windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|p| p + 4)
}

/// Parse Content-Length from raw HTTP header bytes.
fn parse_content_length(headers: &[u8]) -> usize {
    let header_str = String::from_utf8_lossy(headers).to_lowercase();
    for line in header_str.lines() {
        if let Some(val) = line.strip_prefix("content-length:") {
            return val.trim().parse().unwrap_or(0);
        }
    }
    0
}

/// HTTP client for communicating with Cloud-Hypervisor API over Unix socket
pub struct CloudHypervisorClient {
    socket_path: String,
}

impl CloudHypervisorClient {
    pub fn new(socket_path: &str) -> Self {
        Self {
            socket_path: socket_path.to_string(),
        }
    }

    /// Parsed HTTP response with status code and optional body.
    fn send_request(
        &self,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> Result<(u16, Option<String>), HypervisorError> {
        let mut stream = UnixStream::connect(&self.socket_path)
            .map_err(|e| HypervisorError::SocketConnection(e.to_string()))?;

        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .map_err(HypervisorError::ProcessStart)?;

        // Build request matching the official cloud-hypervisor api_client format:
        //   {METHOD} /api/v1/{path} HTTP/1.1\r\nHost: localhost\r\nAccept: */*\r\n
        // With body: add Content-Type and Content-Length headers
        let request = if let Some(body_str) = body {
            format!(
                "{} /api/v1{} HTTP/1.1\r\nHost: localhost\r\nAccept: */*\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                method, path, body_str.len(), body_str
            )
        } else {
            format!(
                "{} /api/v1{} HTTP/1.1\r\nHost: localhost\r\nAccept: */*\r\n\r\n",
                method, path
            )
        };

        stream
            .write_all(request.as_bytes())
            .map_err(HypervisorError::ProcessStart)?;
        stream.flush().map_err(HypervisorError::ProcessStart)?;

        // Read the full response in chunks (matching official client approach)
        let mut raw = Vec::new();
        let mut buf = [0u8; 256];
        loop {
            match stream.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    raw.extend_from_slice(&buf[..n]);
                    // Check if we have a complete response (headers + full body)
                    if let Some(header_end) = find_header_end(&raw) {
                        let content_len = parse_content_length(&raw[..header_end]);
                        if raw.len() >= header_end + content_len {
                            break;
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => break,
                Err(e) => return Err(HypervisorError::ProcessStart(e)),
            }
        }

        let response = String::from_utf8_lossy(&raw);

        // Parse status code from the first line: "HTTP/1.x {code} ..."
        let status_code = response
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|code| code.parse::<u16>().ok())
            .unwrap_or(0);

        // Extract body after the header/body separator
        let body = if let Some(pos) = response.find("\r\n\r\n") {
            let b = &response[pos + 4..];
            if b.is_empty() { None } else { Some(b.to_string()) }
        } else {
            None
        };

        Ok((status_code, body))
    }

    /// Check that the response status indicates success (2xx).
    fn expect_success(
        &self,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> Result<Option<String>, HypervisorError> {
        let (status, response_body) = self.send_request(method, path, body)?;
        if (200..300).contains(&status) {
            Ok(response_body)
        } else {
            Err(HypervisorError::ApiRequest(format!(
                "{} /api/v1{} failed with status {}: {}",
                method,
                path,
                status,
                response_body.unwrap_or_default()
            )))
        }
    }

    pub fn create_vm(&self, config: &VmConfig) -> Result<(), HypervisorError> {
        let vm_config = VmCreateConfig {
            cpus: CpuConfig {
                boot_vcpus: config.vcpu_count,
                max_vcpus: config.vcpu_count,
            },
            memory: MemoryConfig {
                size: (config.mem_size_mib as u64) * 1024 * 1024,
            },
            payload: PayloadConfig {
                firmware: None,
                kernel: config.kernel_image_path.clone(),
                cmdline: config.kernel_args.clone(),
            },
            disks: vec![DiskConfig {
                path: config.rootfs_path.clone(),
            }],
            console: ConsoleConfig {
                mode: "Pty".to_string(),
                file: None,
            },
            serial: ConsoleConfig {
                mode: "Off".to_string(),
                file: None,
            },
        };

        let body = serde_json::to_string(&vm_config)
            .map_err(|e| HypervisorError::ApiRequest(e.to_string()))?;

        self.expect_success("PUT", "/vm.create", Some(&body))?;
        Ok(())
    }

    /// Extract console PTY path from vm.info response
    pub fn get_console_pty_path(&self) -> Result<Option<String>, HypervisorError> {
        let body = self
            .expect_success("GET", "/vm.info", None)?
            .ok_or_else(|| {
                HypervisorError::ApiRequest("vm.info returned no body".to_string())
            })?;

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
            if let Some(config) = json.get("config") {
                if let Some(console) = config.get("console") {
                    if let Some(file) = console.get("file") {
                        if let Some(path) = file.as_str() {
                            return Ok(Some(path.to_string()));
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    pub fn boot_vm(&self) -> Result<(), HypervisorError> {
        self.expect_success("PUT", "/vm.boot", None)?;
        Ok(())
    }

    pub fn pause_vm(&self) -> Result<(), HypervisorError> {
        self.expect_success("PUT", "/vm.pause", None)?;
        Ok(())
    }

    pub fn resume_vm(&self) -> Result<(), HypervisorError> {
        self.expect_success("PUT", "/vm.resume", None)?;
        Ok(())
    }

    pub fn shutdown_vm(&self) -> Result<(), HypervisorError> {
        self.expect_success("PUT", "/vm.shutdown", None)?;
        Ok(())
    }
}

/// Manages a running Cloud-Hypervisor process
pub struct CloudHypervisorProcessHandle {
    child: Mutex<Option<Child>>,
    socket_path: String,
    console_socket_path: String,
    log_path: String,
    running: Arc<AtomicBool>,
    console_thread: Mutex<Option<thread::JoinHandle<()>>>,
}

impl CloudHypervisorProcessHandle {
    pub fn spawn(
        socket_path: &str,
        console_socket_path: &str,
        log_path: &str,
    ) -> Result<Self, HypervisorError> {
        // Remove existing sockets if present
        let _ = std::fs::remove_file(socket_path);
        let _ = std::fs::remove_file(console_socket_path);

        // Create/truncate log file
        let _log_file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(log_path)?;

        // Spawn cloud-hypervisor with API socket
        let child = Command::new("cloud-hypervisor")
            .arg("--api-socket")
            .arg(socket_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        let running = Arc::new(AtomicBool::new(true));

        // Wait for API socket to be available
        for _ in 0..50 {
            if std::path::Path::new(socket_path).exists() {
                return Ok(Self {
                    child: Mutex::new(Some(child)),
                    socket_path: socket_path.to_string(),
                    console_socket_path: console_socket_path.to_string(),
                    log_path: log_path.to_string(),
                    running,
                    console_thread: Mutex::new(None),
                });
            }
            std::thread::sleep(Duration::from_millis(100));
        }

        // Cleanup on timeout
        running.store(false, Ordering::SeqCst);
        let mut child = child;
        let _ = child.kill();
        let _ = child.wait();
        let _ = std::fs::remove_file(socket_path);

        Err(HypervisorError::Timeout(
            "Socket not available after timeout".to_string(),
        ))
    }

    /// Start the console proxy thread that bridges the PTY to a Unix socket
    pub fn start_console_proxy(&self, pty_path: &str) -> Result<(), HypervisorError> {
        // Remove existing console socket if present
        let _ = std::fs::remove_file(&self.console_socket_path);

        // Open the PTY
        let pty_fd = OpenOptions::new()
            .read(true)
            .write(true)
            .open(pty_path)
            .map_err(|e| {
                HypervisorError::SocketConnection(format!("Failed to open PTY {}: {}", pty_path, e))
            })?;

        // Create Unix socket for console connections
        let console_listener = UnixListener::bind(&self.console_socket_path).map_err(|e| {
            HypervisorError::SocketConnection(format!("Failed to create console socket: {}", e))
        })?;
        console_listener.set_nonblocking(true).map_err(|e| {
            HypervisorError::SocketConnection(format!("Failed to set non-blocking: {}", e))
        })?;

        // Open log file for writing
        let log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)?;

        let running_clone = self.running.clone();
        let log_path_clone = self.log_path.clone();

        // Spawn thread to handle console I/O
        let console_thread = thread::spawn(move || {
            Self::console_proxy_loop(pty_fd, console_listener, log_file, &log_path_clone, running_clone);
        });

        *self.console_thread.lock().unwrap() = Some(console_thread);
        Ok(())
    }

    fn console_proxy_loop(
        pty_file: File,
        listener: UnixListener,
        mut log_file: File,
        log_path: &str,
        running: Arc<AtomicBool>,
    ) {
        let pty_raw = pty_file.as_raw_fd();
        let mut clients: Vec<UnixStream> = Vec::new();
        let mut buf = [0u8; 4096];

        // Set PTY to non-blocking
        unsafe {
            let flags = libc::fcntl(pty_raw, libc::F_GETFL);
            libc::fcntl(pty_raw, libc::F_SETFL, flags | libc::O_NONBLOCK);
        }

        while running.load(Ordering::SeqCst) {
            // Accept new client connections
            if let Ok((stream, _)) = listener.accept() {
                stream.set_nonblocking(true).ok();
                // Send existing log content to new client
                if let Ok(mut existing_log) = File::open(log_path) {
                    let mut log_content = Vec::new();
                    if existing_log.read_to_end(&mut log_content).is_ok() && !log_content.is_empty()
                    {
                        let mut s = stream.try_clone().unwrap();
                        let _ = s.write_all(&log_content);
                    }
                }
                clients.push(stream);
            }

            // Read from PTY and broadcast to clients + log file
            let mut pty_reader = unsafe { File::from_raw_fd(libc::dup(pty_raw)) };
            match pty_reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let data = &buf[..n];

                    let _ = log_file.write_all(data);
                    let _ = log_file.flush();

                    clients.retain_mut(|client| client.write_all(data).is_ok());
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(_) => break,
            }

            // Read from clients and write to PTY
            for client in &mut clients {
                match client.read(&mut buf) {
                    Ok(0) => {}
                    Ok(n) => {
                        let mut pty_writer = unsafe { File::from_raw_fd(libc::dup(pty_raw)) };
                        let _ = pty_writer.write_all(&buf[..n]);
                        let _ = pty_writer.flush();
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(_) => {}
                }
            }

            thread::sleep(Duration::from_millis(10));
        }
    }
}

/// Cloud-Hypervisor instance that implements HypervisorProcess
pub struct CloudHypervisorInstance {
    process: CloudHypervisorProcessHandle,
    client: CloudHypervisorClient,
}

impl CloudHypervisorInstance {
    pub fn new(process: CloudHypervisorProcessHandle) -> Self {
        let client = CloudHypervisorClient::new(&process.socket_path);
        Self { process, client }
    }
}

impl HypervisorProcess for CloudHypervisorInstance {
    fn configure(&self, config: &VmConfig) -> Result<(), HypervisorError> {
        self.client.create_vm(config)?;
        Ok(())
    }

    fn start(&self) -> Result<(), HypervisorError> {
        self.client.boot_vm()?;

        // The console PTY is allocated during vm.boot (device creation),
        // not during vm.create. Poll for the PTY path to become available.
        for _ in 0..30 {
            match self.client.get_console_pty_path() {
                Ok(Some(pty_path)) => {
                    self.process.start_console_proxy(&pty_path)?;
                    return Ok(());
                }
                Ok(None) => {
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(_) => {
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        }

        Err(HypervisorError::Timeout(
            "Console PTY path not available after boot".to_string(),
        ))
    }

    fn pause(&self) -> Result<(), HypervisorError> {
        self.client.pause_vm()
    }

    fn resume(&self) -> Result<(), HypervisorError> {
        self.client.resume_vm()
    }

    fn kill(&self) -> Result<(), HypervisorError> {
        self.process.running.store(false, Ordering::SeqCst);

        // Try graceful shutdown first
        let _ = self.client.shutdown_vm();

        // Then force kill if needed
        if let Some(mut child) = self.process.child.lock().unwrap().take() {
            let _ = child.kill();
            let _ = child.wait();
        }

        // Wait for console thread to finish
        if let Some(handle) = self.process.console_thread.lock().unwrap().take() {
            let _ = handle.join();
        }

        let _ = std::fs::remove_file(&self.process.socket_path);
        let _ = std::fs::remove_file(&self.process.console_socket_path);
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.process.running.load(Ordering::SeqCst)
    }

    fn socket_path(&self) -> &str {
        &self.process.socket_path
    }

    fn console_socket_path(&self) -> &str {
        &self.process.console_socket_path
    }

    fn log_path(&self) -> &str {
        &self.process.log_path
    }
}

/// Cloud-Hypervisor backend factory
pub struct CloudHypervisorBackend;

impl Hypervisor for CloudHypervisorBackend {
    fn spawn(
        &self,
        socket_path: &str,
        console_socket_path: &str,
        log_path: &str,
    ) -> Result<Box<dyn HypervisorProcess>, HypervisorError> {
        let process =
            CloudHypervisorProcessHandle::spawn(socket_path, console_socket_path, log_path)?;
        Ok(Box::new(CloudHypervisorInstance::new(process)))
    }

    fn hypervisor_type(&self) -> HypervisorType {
        HypervisorType::CloudHypervisor
    }

    fn is_available(&self) -> bool {
        Command::new("cloud-hypervisor")
            .arg("--version")
            .output()
            .is_ok()
    }
}
