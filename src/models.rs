use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum VmState {
    Created,
    Running,
    Paused,
    Stopped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmConfig {
    pub vcpu_count: u8,
    pub mem_size_mib: u32,
    pub kernel_image_path: String,
    pub rootfs_path: String,
    pub kernel_args: String,
}

/// Default kernel arguments for Firecracker VMs
pub const DEFAULT_KERNEL_ARGS: &str = "console=ttyS0 reboot=k panic=1 pci=off";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vm {
    pub id: String,
    pub name: String,
    pub state: VmState,
    pub config: VmConfig,
    pub socket_path: String,
    pub console_socket_path: String,
    pub log_path: String,
}

impl Vm {
    pub fn new(name: String, config: VmConfig) -> Self {
        let id = Uuid::new_v4().to_string();
        let socket_path = format!("/tmp/firecracker-{}.sock", id);
        let console_socket_path = format!("/tmp/firecracker-{}.console.sock", id);
        let log_path = format!("/tmp/firecracker-{}.log", id);
        Self {
            id,
            name,
            state: VmState::Created,
            config,
            socket_path,
            console_socket_path,
            log_path,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateVmRequest {
    pub name: String,
    pub vcpu_count: u8,
    pub mem_size_mib: u32,
    pub kernel_image_path: String,
    pub rootfs_path: String,
    #[serde(default)]
    pub kernel_args: Option<String>,
}

impl From<CreateVmRequest> for VmConfig {
    fn from(req: CreateVmRequest) -> Self {
        VmConfig {
            vcpu_count: req.vcpu_count,
            mem_size_mib: req.mem_size_mib,
            kernel_image_path: req.kernel_image_path,
            rootfs_path: req.rootfs_path,
            kernel_args: req.kernel_args.unwrap_or_else(|| DEFAULT_KERNEL_ARGS.to_string()),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct VmResponse {
    pub id: String,
    pub name: String,
    pub state: VmState,
    pub vcpu_count: u8,
    pub mem_size_mib: u32,
    pub console_socket_path: String,
    pub log_path: String,
}

impl From<&Vm> for VmResponse {
    fn from(vm: &Vm) -> Self {
        VmResponse {
            id: vm.id.clone(),
            name: vm.name.clone(),
            state: vm.state.clone(),
            vcpu_count: vm.config.vcpu_count,
            mem_size_mib: vm.config.mem_size_mib,
            console_socket_path: vm.console_socket_path.clone(),
            log_path: vm.log_path.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: String,
    pub message: String,
}

impl ApiError {
    pub fn new(error: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            message: message.into(),
        }
    }
}
