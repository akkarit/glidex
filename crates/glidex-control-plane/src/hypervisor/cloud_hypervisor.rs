use super::{Hypervisor, HypervisorError, HypervisorProcess, HypervisorType};
use crate::models::VmConfig;
use serde::Serialize;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
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

    fn send_request(
        &self,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> Result<String, HypervisorError> {
        let stream = UnixStream::connect(&self.socket_path)
            .map_err(|e| HypervisorError::SocketConnection(e.to_string()))?;

        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .map_err(HypervisorError::ProcessStart)?;

        let mut writer = stream
            .try_clone()
            .map_err(HypervisorError::ProcessStart)?;
        let mut reader = BufReader::new(stream);

        let body_str = body.unwrap_or("");
        let content_length = body_str.len();

        // Cloud-Hypervisor uses /api/v1/ prefix
        let request = format!(
            "{} /api/v1{} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            method, path, content_length, body_str
        );

        writer
            .write_all(request.as_bytes())
            .map_err(HypervisorError::ProcessStart)?;
        writer.flush().map_err(HypervisorError::ProcessStart)?;

        let mut response = String::new();
        let mut content_length: usize = 0;

        loop {
            let mut line = String::new();
            reader
                .read_line(&mut line)
                .map_err(HypervisorError::ProcessStart)?;
            response.push_str(&line);

            if line.to_lowercase().starts_with("content-length:") {
                if let Some(len_str) = line.split(':').nth(1) {
                    content_length = len_str.trim().parse().unwrap_or(0);
                }
            }

            if line == "\r\n" || line == "\n" {
                break;
            }
        }

        if content_length > 0 {
            let mut body_buf = vec![0u8; content_length];
            reader
                .read_exact(&mut body_buf)
                .map_err(HypervisorError::ProcessStart)?;
            response.push_str(&String::from_utf8_lossy(&body_buf));
        }

        Ok(response)
    }

    pub fn create_vm(&self, config: &VmConfig, log_path: &str) -> Result<(), HypervisorError> {
        let vm_config = VmCreateConfig {
            cpus: CpuConfig {
                boot_vcpus: config.vcpu_count,
                max_vcpus: config.vcpu_count,
            },
            memory: MemoryConfig {
                size: (config.mem_size_mib as u64) * 1024 * 1024,
            },
            payload: PayloadConfig {
                kernel: config.kernel_image_path.clone(),
                cmdline: config.kernel_args.clone(),
            },
            disks: vec![DiskConfig {
                path: config.rootfs_path.clone(),
            }],
            console: ConsoleConfig {
                mode: "File".to_string(),
                file: Some(log_path.to_string()),
            },
            serial: ConsoleConfig {
                mode: "Off".to_string(),
                file: None,
            },
        };

        let body = serde_json::to_string(&vm_config)
            .map_err(|e| HypervisorError::ApiRequest(e.to_string()))?;

        let response = self.send_request("PUT", "/vm.create", Some(&body))?;

        if !response.contains("HTTP/1.1 204") && !response.contains("HTTP/1.1 200") {
            return Err(HypervisorError::ApiRequest(format!(
                "Failed to create VM: {}",
                response
            )));
        }

        Ok(())
    }

    pub fn boot_vm(&self) -> Result<(), HypervisorError> {
        let response = self.send_request("PUT", "/vm.boot", None)?;

        if !response.contains("HTTP/1.1 204") && !response.contains("HTTP/1.1 200") {
            return Err(HypervisorError::ApiRequest(format!(
                "Failed to boot VM: {}",
                response
            )));
        }

        Ok(())
    }

    pub fn pause_vm(&self) -> Result<(), HypervisorError> {
        let response = self.send_request("PUT", "/vm.pause", None)?;

        if !response.contains("HTTP/1.1 204") && !response.contains("HTTP/1.1 200") {
            return Err(HypervisorError::ApiRequest(format!(
                "Failed to pause VM: {}",
                response
            )));
        }

        Ok(())
    }

    pub fn resume_vm(&self) -> Result<(), HypervisorError> {
        let response = self.send_request("PUT", "/vm.resume", None)?;

        if !response.contains("HTTP/1.1 204") && !response.contains("HTTP/1.1 200") {
            return Err(HypervisorError::ApiRequest(format!(
                "Failed to resume VM: {}",
                response
            )));
        }

        Ok(())
    }

    pub fn shutdown_vm(&self) -> Result<(), HypervisorError> {
        let response = self.send_request("PUT", "/vm.shutdown", None)?;

        if !response.contains("HTTP/1.1 204") && !response.contains("HTTP/1.1 200") {
            return Err(HypervisorError::ApiRequest(format!(
                "Failed to shutdown VM: {}",
                response
            )));
        }

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
        // Cloud-Hypervisor uses vm.create to configure
        self.client.create_vm(config, &self.process.log_path)
    }

    fn start(&self) -> Result<(), HypervisorError> {
        self.client.boot_vm()
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
