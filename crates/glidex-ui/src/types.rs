use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum VmState {
    Created,
    Running,
    Paused,
    Stopped,
}

impl VmState {
    pub fn css_class(&self) -> &'static str {
        match self {
            VmState::Running => "bg-green-500",
            VmState::Stopped => "bg-red-500",
            VmState::Paused => "bg-yellow-500",
            VmState::Created => "bg-blue-500",
        }
    }

    pub fn display(&self) -> &'static str {
        match self {
            VmState::Running => "Running",
            VmState::Stopped => "Stopped",
            VmState::Paused => "Paused",
            VmState::Created => "Created",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmResponse {
    pub id: String,
    pub name: String,
    pub state: VmState,
    pub vcpu_count: u8,
    pub mem_size_mib: u32,
    pub console_socket_path: String,
    pub log_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateVmRequest {
    pub name: String,
    pub vcpu_count: u8,
    pub mem_size_mib: u32,
    pub kernel_image_path: String,
    pub rootfs_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kernel_args: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub error: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
}
