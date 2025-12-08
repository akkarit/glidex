use clap::Parser;
use colored::Colorize;
use nix::sys::termios::{self, LocalFlags, SetArg, Termios};
use reqwest::Client;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use tabled::{Table, Tabled};

#[derive(Parser)]
#[command(name = "gxctl")]
#[command(about = "Interactive CLI for Glidex Control Plane")]
struct Cli {
    /// API server URL
    #[arg(short, long, default_value = "http://localhost:8080")]
    server: String,
}

#[derive(Debug, Deserialize, Tabled)]
struct VmResponse {
    id: String,
    name: String,
    state: String,
    vcpu_count: u8,
    mem_size_mib: u32,
}

#[derive(Debug, Serialize)]
struct CreateVmRequest {
    name: String,
    vcpu_count: u8,
    mem_size_mib: u32,
    kernel_image_path: String,
    rootfs_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    kernel_args: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    error: String,
    message: String,
}

#[derive(Debug, Deserialize)]
struct ConsoleInfo {
    #[allow(dead_code)]
    vm_id: String,
    console_socket_path: String,
    log_path: String,
    available: bool,
}

struct CliClient {
    client: Client,
    base_url: String,
}

impl CliClient {
    fn new(base_url: String) -> Self {
        Self {
            client: Client::new(),
            base_url,
        }
    }

    async fn list_vms(&self) -> Result<Vec<VmResponse>, String> {
        let resp = self
            .client
            .get(format!("{}/vms", self.base_url))
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if resp.status().is_success() {
            resp.json()
                .await
                .map_err(|e| format!("Failed to parse response: {}", e))
        } else {
            let error: ApiError = resp
                .json()
                .await
                .map_err(|e| format!("Failed to parse error: {}", e))?;
            Err(format!("{}: {}", error.error, error.message))
        }
    }

    async fn get_vm(&self, id: &str) -> Result<VmResponse, String> {
        let resp = self
            .client
            .get(format!("{}/vms/{}", self.base_url, id))
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if resp.status().is_success() {
            resp.json()
                .await
                .map_err(|e| format!("Failed to parse response: {}", e))
        } else {
            let error: ApiError = resp
                .json()
                .await
                .map_err(|e| format!("Failed to parse error: {}", e))?;
            Err(format!("{}: {}", error.error, error.message))
        }
    }

    async fn create_vm(&self, request: CreateVmRequest) -> Result<VmResponse, String> {
        let resp = self
            .client
            .post(format!("{}/vms", self.base_url))
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if resp.status().is_success() {
            resp.json()
                .await
                .map_err(|e| format!("Failed to parse response: {}", e))
        } else {
            let error: ApiError = resp
                .json()
                .await
                .map_err(|e| format!("Failed to parse error: {}", e))?;
            Err(format!("{}: {}", error.error, error.message))
        }
    }

    async fn start_vm(&self, id: &str) -> Result<VmResponse, String> {
        let resp = self
            .client
            .post(format!("{}/vms/{}/start", self.base_url, id))
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if resp.status().is_success() {
            resp.json()
                .await
                .map_err(|e| format!("Failed to parse response: {}", e))
        } else {
            let error: ApiError = resp
                .json()
                .await
                .map_err(|e| format!("Failed to parse error: {}", e))?;
            Err(format!("{}: {}", error.error, error.message))
        }
    }

    async fn stop_vm(&self, id: &str) -> Result<VmResponse, String> {
        let resp = self
            .client
            .post(format!("{}/vms/{}/stop", self.base_url, id))
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if resp.status().is_success() {
            resp.json()
                .await
                .map_err(|e| format!("Failed to parse response: {}", e))
        } else {
            let error: ApiError = resp
                .json()
                .await
                .map_err(|e| format!("Failed to parse error: {}", e))?;
            Err(format!("{}: {}", error.error, error.message))
        }
    }

    async fn pause_vm(&self, id: &str) -> Result<VmResponse, String> {
        let resp = self
            .client
            .post(format!("{}/vms/{}/pause", self.base_url, id))
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if resp.status().is_success() {
            resp.json()
                .await
                .map_err(|e| format!("Failed to parse response: {}", e))
        } else {
            let error: ApiError = resp
                .json()
                .await
                .map_err(|e| format!("Failed to parse error: {}", e))?;
            Err(format!("{}: {}", error.error, error.message))
        }
    }

    async fn delete_vm(&self, id: &str) -> Result<(), String> {
        let resp = self
            .client
            .delete(format!("{}/vms/{}", self.base_url, id))
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if resp.status().is_success() {
            Ok(())
        } else {
            let error: ApiError = resp
                .json()
                .await
                .map_err(|e| format!("Failed to parse error: {}", e))?;
            Err(format!("{}: {}", error.error, error.message))
        }
    }

    async fn health_check(&self) -> Result<(), String> {
        let resp = self
            .client
            .get(format!("{}/health", self.base_url))
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if resp.status().is_success() {
            Ok(())
        } else {
            Err("Health check failed".to_string())
        }
    }

    async fn get_console_info(&self, id: &str) -> Result<ConsoleInfo, String> {
        let resp = self
            .client
            .get(format!("{}/vms/{}/console", self.base_url, id))
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if resp.status().is_success() {
            resp.json()
                .await
                .map_err(|e| format!("Failed to parse response: {}", e))
        } else {
            let error: ApiError = resp
                .json()
                .await
                .map_err(|e| format!("Failed to parse error: {}", e))?;
            Err(format!("{}: {}", error.error, error.message))
        }
    }

    /// Resolve a VM identifier (name or ID) to an ID.
    /// First tries to use it as an ID, then searches by name.
    async fn resolve_vm(&self, name_or_id: &str) -> Result<String, String> {
        // First, try to get VM by ID directly
        if let Ok(vm) = self.get_vm(name_or_id).await {
            return Ok(vm.id);
        }

        // If that fails, search by name
        let vms = self.list_vms().await?;
        let matches: Vec<_> = vms.iter().filter(|vm| vm.name == name_or_id).collect();

        match matches.len() {
            0 => Err(format!("VM '{}' not found", name_or_id)),
            1 => Ok(matches[0].id.clone()),
            _ => {
                let ids: Vec<_> = matches.iter().map(|vm| vm.id.as_str()).collect();
                Err(format!(
                    "Multiple VMs found with name '{}'. Use ID instead: {}",
                    name_or_id,
                    ids.join(", ")
                ))
            }
        }
    }
}

fn print_help() {
    println!("{}", "Available commands:".bold());
    println!("  {}              - List all VMs", "list".cyan());
    println!("  {}    - Show VM details", "get <name|id>".cyan());
    println!(
        "  {}           - Create a new VM (interactive)",
        "create".cyan()
    );
    println!("  {}  - Start a VM", "start <name|id>".cyan());
    println!("  {}   - Stop a VM", "stop <name|id>".cyan());
    println!("  {}  - Pause a VM", "pause <name|id>".cyan());
    println!("  {} - Connect to VM console (interactive)", "connect <name|id>".cyan());
    println!("  {}     - Show VM serial console log", "log <name|id>".cyan());
    println!("  {} - Delete a VM", "delete <name|id>".cyan());
    println!("  {}            - Check API server health", "health".cyan());
    println!("  {}              - Show this help", "help".cyan());
    println!("  {}              - Exit the CLI", "exit".cyan());
    println!();
    println!(
        "{} You can use either VM name or ID for commands.",
        "Note:".dimmed()
    );
}

fn prompt(msg: &str) -> String {
    print!("{}", msg);
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    input.trim().to_string()
}

fn prompt_optional(msg: &str) -> Option<String> {
    let input = prompt(msg);
    if input.is_empty() {
        None
    } else {
        Some(input)
    }
}

async fn handle_create(client: &CliClient) {
    println!("{}", "Create new VM".bold());
    println!("{}", "-".repeat(40));

    let name = prompt("VM name: ");
    if name.is_empty() {
        println!("{}", "Error: VM name is required".red());
        return;
    }

    let vcpu_count: u8 = match prompt("vCPU count [1]: ").parse() {
        Ok(n) => n,
        Err(_) => 1,
    };

    let mem_size_mib: u32 = match prompt("Memory (MiB) [512]: ").parse() {
        Ok(n) => n,
        Err(_) => 512,
    };

    let home_dir = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let default_kernel = format!("{}/.glidex/vmlinux.bin", home_dir);
    let default_rootfs = format!("{}/.glidex/rootfs.ext4", home_dir);

    let kernel_image_path = match prompt(&format!("Kernel image path [{}]: ", default_kernel)).as_str() {
        "" => default_kernel,
        s => s.to_string(),
    };

    let rootfs_path = match prompt(&format!("Root filesystem path [{}]: ", default_rootfs)).as_str() {
        "" => default_rootfs,
        s => s.to_string(),
    };

    let kernel_args = prompt_optional("Kernel arguments (optional, default: console=ttyS0 reboot=k panic=1 pci=off): ");

    let request = CreateVmRequest {
        name,
        vcpu_count,
        mem_size_mib,
        kernel_image_path,
        rootfs_path,
        kernel_args,
    };

    match client.create_vm(request).await {
        Ok(vm) => {
            println!("{}", "VM created successfully!".green());
            println!("  ID: {}", vm.id.yellow());
            println!("  Name: {}", vm.name);
            println!("  State: {}", vm.state);
        }
        Err(e) => println!("{} {}", "Error:".red(), e),
    }
}

fn format_state(state: &str) -> String {
    match state {
        "running" => state.green().to_string(),
        "stopped" => state.red().to_string(),
        "paused" => state.yellow().to_string(),
        "created" => state.blue().to_string(),
        _ => state.to_string(),
    }
}

/// Set terminal to raw mode for interactive console
fn set_raw_mode(fd: BorrowedFd<'_>) -> Option<Termios> {
    let orig_termios = termios::tcgetattr(fd).ok()?;
    let mut raw = orig_termios.clone();

    // Disable canonical mode and echo
    raw.local_flags.remove(LocalFlags::ICANON);
    raw.local_flags.remove(LocalFlags::ECHO);
    raw.local_flags.remove(LocalFlags::ISIG);

    termios::tcsetattr(fd, SetArg::TCSANOW, &raw).ok()?;
    Some(orig_termios)
}

/// Restore terminal to original mode
fn restore_terminal(fd: BorrowedFd<'_>, termios: &Termios) {
    let _ = termios::tcsetattr(fd, SetArg::TCSANOW, termios);
}

async fn handle_log(client: &CliClient, vm_id: &str) {
    // Get console info from API
    let console_info = match client.get_console_info(vm_id).await {
        Ok(info) => info,
        Err(e) => {
            println!("{} {}", "Error:".red(), e);
            return;
        }
    };

    let log_path = &console_info.log_path;

    // Try to open and read the log file
    match File::open(log_path) {
        Ok(file) => {
            let reader = BufReader::new(file);
            let mut has_content = false;

            for line in reader.lines() {
                match line {
                    Ok(l) => {
                        println!("{}", l);
                        has_content = true;
                    }
                    Err(e) => {
                        println!("{} Error reading log: {}", "Error:".red(), e);
                        return;
                    }
                }
            }

            if !has_content {
                println!("{} Log file is empty. Start the VM to see console output.", "Info:".yellow());
            }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            println!("{} Log file not found. Start the VM first.", "Info:".yellow());
        }
        Err(e) => {
            println!("{} Failed to open log file: {}", "Error:".red(), e);
        }
    }
}

async fn handle_connect(client: &CliClient, vm_id: &str) {
    // Get console info from API
    let console_info = match client.get_console_info(vm_id).await {
        Ok(info) => info,
        Err(e) => {
            println!("{} {}", "Error:".red(), e);
            return;
        }
    };

    if !console_info.available {
        println!(
            "{} VM is not running. Start the VM first with: start {}",
            "Error:".red(),
            vm_id
        );
        return;
    }

    let socket_path = &console_info.console_socket_path;

    println!(
        "{} Connecting to VM console via {}",
        "Info:".cyan(),
        socket_path
    );
    println!(
        "{} Press {} to detach from console\n",
        "Tip:".yellow(),
        "Ctrl+]".bold()
    );

    // Connect to the console Unix socket
    let stream = match UnixStream::connect(socket_path) {
        Ok(s) => s,
        Err(e) => {
            println!(
                "{} Failed to connect to console socket {}: {}",
                "Error:".red(),
                socket_path,
                e
            );
            return;
        }
    };

    // Set socket to non-blocking for the reader
    stream.set_nonblocking(true).ok();
    let stream_write = match stream.try_clone() {
        Ok(s) => s,
        Err(e) => {
            println!("{} Failed to clone socket: {}", "Error:".red(), e);
            return;
        }
    };

    // Set up signal handler for Ctrl+C (we'll handle Ctrl+] for detach)
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    // Save original terminal settings and set raw mode
    let stdin = io::stdin();
    let stdin_fd = stdin.as_fd();
    let orig_termios = match set_raw_mode(stdin_fd) {
        Some(t) => t,
        None => {
            println!("{} Failed to set terminal to raw mode", "Error:".red());
            return;
        }
    };

    // Spawn thread to read from socket and write to stdout
    let running_reader = running.clone();
    let reader_handle = thread::spawn(move || {
        let mut stream = stream;
        let mut buf = [0u8; 1024];

        while running_reader.load(Ordering::SeqCst) {
            match stream.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let _ = io::stdout().write_all(&buf[..n]);
                    let _ = io::stdout().flush();
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });

    // Spawn thread to read from stdin and write to socket
    let running_writer = running.clone();
    let writer_handle = thread::spawn(move || {
        let mut stream = stream_write;
        let mut buf = [0u8; 1];

        while running_writer.load(Ordering::SeqCst) {
            match io::stdin().read(&mut buf) {
                Ok(0) => break,
                Ok(1) => {
                    // Check for Ctrl+] (0x1d) to detach
                    if buf[0] == 0x1d {
                        running_writer.store(false, Ordering::SeqCst);
                        break;
                    }
                    let _ = stream.write_all(&buf);
                    let _ = stream.flush();
                }
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });

    // Handle Ctrl+C gracefully
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })
    .ok();

    // Wait for threads to finish
    let _ = writer_handle.join();
    running.store(false, Ordering::SeqCst);
    let _ = reader_handle.join();

    // Restore terminal
    restore_terminal(stdin.as_fd(), &orig_termios);

    println!("\n{} Detached from console", "Info:".cyan());
}

async fn handle_command(line: &str, client: &CliClient) -> bool {
    let parts: Vec<&str> = line.trim().split_whitespace().collect();
    if parts.is_empty() {
        return true;
    }

    match parts[0] {
        "help" | "?" => print_help(),

        "exit" | "quit" | "q" => return false,

        "list" | "ls" => match client.list_vms().await {
            Ok(vms) => {
                if vms.is_empty() {
                    println!("{}", "No VMs found".yellow());
                } else {
                    let table = Table::new(&vms).to_string();
                    println!("{}", table);
                }
            }
            Err(e) => println!("{} {}", "Error:".red(), e),
        },

        "get" => {
            if parts.len() < 2 {
                println!("{}", "Usage: get <name|id>".yellow());
                return true;
            }
            let vm_id = match client.resolve_vm(parts[1]).await {
                Ok(id) => id,
                Err(e) => {
                    println!("{} {}", "Error:".red(), e);
                    return true;
                }
            };
            match client.get_vm(&vm_id).await {
                Ok(vm) => {
                    println!("{}", "VM Details".bold());
                    println!("{}", "-".repeat(40));
                    println!("  ID:      {}", vm.id.yellow());
                    println!("  Name:    {}", vm.name);
                    println!("  State:   {}", format_state(&vm.state));
                    println!("  vCPUs:   {}", vm.vcpu_count);
                    println!("  Memory:  {} MiB", vm.mem_size_mib);
                }
                Err(e) => println!("{} {}", "Error:".red(), e),
            }
        }

        "create" => handle_create(client).await,

        "start" => {
            if parts.len() < 2 {
                println!("{}", "Usage: start <name|id>".yellow());
                return true;
            }
            let vm_id = match client.resolve_vm(parts[1]).await {
                Ok(id) => id,
                Err(e) => {
                    println!("{} {}", "Error:".red(), e);
                    return true;
                }
            };
            match client.start_vm(&vm_id).await {
                Ok(vm) => {
                    println!(
                        "{} VM {} is now {}",
                        "Success:".green(),
                        vm.name,
                        format_state(&vm.state)
                    );
                }
                Err(e) => println!("{} {}", "Error:".red(), e),
            }
        }

        "stop" => {
            if parts.len() < 2 {
                println!("{}", "Usage: stop <name|id>".yellow());
                return true;
            }
            let vm_id = match client.resolve_vm(parts[1]).await {
                Ok(id) => id,
                Err(e) => {
                    println!("{} {}", "Error:".red(), e);
                    return true;
                }
            };
            match client.stop_vm(&vm_id).await {
                Ok(vm) => {
                    println!(
                        "{} VM {} is now {}",
                        "Success:".green(),
                        vm.name,
                        format_state(&vm.state)
                    );
                }
                Err(e) => println!("{} {}", "Error:".red(), e),
            }
        }

        "pause" => {
            if parts.len() < 2 {
                println!("{}", "Usage: pause <name|id>".yellow());
                return true;
            }
            let vm_id = match client.resolve_vm(parts[1]).await {
                Ok(id) => id,
                Err(e) => {
                    println!("{} {}", "Error:".red(), e);
                    return true;
                }
            };
            match client.pause_vm(&vm_id).await {
                Ok(vm) => {
                    println!(
                        "{} VM {} is now {}",
                        "Success:".green(),
                        vm.name,
                        format_state(&vm.state)
                    );
                }
                Err(e) => println!("{} {}", "Error:".red(), e),
            }
        }

        "connect" | "console" | "attach" => {
            if parts.len() < 2 {
                println!("{}", "Usage: connect <name|id>".yellow());
                return true;
            }
            let vm_id = match client.resolve_vm(parts[1]).await {
                Ok(id) => id,
                Err(e) => {
                    println!("{} {}", "Error:".red(), e);
                    return true;
                }
            };
            handle_connect(client, &vm_id).await;
        }

        "log" | "logs" => {
            if parts.len() < 2 {
                println!("{}", "Usage: log <name|id>".yellow());
                return true;
            }
            let vm_id = match client.resolve_vm(parts[1]).await {
                Ok(id) => id,
                Err(e) => {
                    println!("{} {}", "Error:".red(), e);
                    return true;
                }
            };
            handle_log(client, &vm_id).await;
        }

        "delete" | "rm" => {
            if parts.len() < 2 {
                println!("{}", "Usage: delete <name|id>".yellow());
                return true;
            }
            let vm_id = match client.resolve_vm(parts[1]).await {
                Ok(id) => id,
                Err(e) => {
                    println!("{} {}", "Error:".red(), e);
                    return true;
                }
            };
            let confirm = prompt(&format!(
                "Are you sure you want to delete VM {}? [y/N]: ",
                parts[1]
            ));
            if confirm.to_lowercase() == "y" {
                match client.delete_vm(&vm_id).await {
                    Ok(()) => println!("{} VM deleted", "Success:".green()),
                    Err(e) => println!("{} {}", "Error:".red(), e),
                }
            } else {
                println!("Cancelled");
            }
        }

        "health" => match client.health_check().await {
            Ok(()) => println!("{} API server is healthy", "OK:".green()),
            Err(e) => println!("{} {}", "Error:".red(), e),
        },

        _ => println!(
            "{} Unknown command: {}. Type 'help' for available commands.",
            "Error:".red(),
            parts[0]
        ),
    }

    true
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let client = CliClient::new(cli.server.clone());

    println!(
        "{}",
        r#"
   _____ _ _     _
  / ____| (_)   | |
 | |  __| |_  __| | _____  __
 | | |_ | | |/ _` |/ _ \ \/ /
 | |__| | | | (_| |  __/>  <
  \_____|_|_|\__,_|\___/_/\_\
          Control Plane CLI
"#
        .cyan()
    );

    println!("Connected to: {}", cli.server.yellow());
    println!("Type {} for available commands\n", "help".cyan());

    let mut rl = DefaultEditor::new().expect("Failed to initialize readline");

    loop {
        match rl.readline("gxctl> ") {
            Ok(line) => {
                if line.trim().is_empty() {
                    continue;
                }
                let _ = rl.add_history_entry(&line);
                if !handle_command(&line, &client).await {
                    println!("Goodbye!");
                    break;
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("Use 'exit' to quit");
            }
            Err(ReadlineError::Eof) => {
                println!("Goodbye!");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }
}
