use crate::firecracker::{FirecrackerError, FirecrackerProcess};
use crate::models::{Vm, VmConfig, VmState};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug)]
pub enum VmManagerError {
    VmNotFound(String),
    VmAlreadyExists(String),
    InvalidState { current: VmState, operation: String },
    FirecrackerError(FirecrackerError),
}

impl std::fmt::Display for VmManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VmManagerError::VmNotFound(id) => write!(f, "VM not found: {}", id),
            VmManagerError::VmAlreadyExists(name) => write!(f, "VM already exists: {}", name),
            VmManagerError::InvalidState { current, operation } => {
                write!(f, "Invalid state {:?} for operation: {}", current, operation)
            }
            VmManagerError::FirecrackerError(e) => write!(f, "Firecracker error: {}", e),
        }
    }
}

impl From<FirecrackerError> for VmManagerError {
    fn from(e: FirecrackerError) -> Self {
        VmManagerError::FirecrackerError(e)
    }
}

struct VmEntry {
    vm: Vm,
    process: Option<FirecrackerProcess>,
}

pub struct VmManager {
    vms: RwLock<HashMap<String, VmEntry>>,
}

impl VmManager {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            vms: RwLock::new(HashMap::new()),
        })
    }

    pub async fn create_vm(&self, name: String, config: VmConfig) -> Result<Vm, VmManagerError> {
        let mut vms = self.vms.write().await;

        // Check if VM with same name exists
        if vms.values().any(|entry| entry.vm.name == name) {
            return Err(VmManagerError::VmAlreadyExists(name));
        }

        let vm = Vm::new(name, config);
        let vm_clone = vm.clone();

        vms.insert(
            vm.id.clone(),
            VmEntry {
                vm,
                process: None,
            },
        );

        Ok(vm_clone)
    }

    pub async fn start_vm(&self, vm_id: &str) -> Result<Vm, VmManagerError> {
        let mut vms = self.vms.write().await;

        let entry = vms
            .get_mut(vm_id)
            .ok_or_else(|| VmManagerError::VmNotFound(vm_id.to_string()))?;

        match entry.vm.state {
            VmState::Created | VmState::Stopped => {
                // Spawn Firecracker process with console socket and log file
                let process = FirecrackerProcess::spawn(
                    &entry.vm.socket_path,
                    &entry.vm.console_socket_path,
                    &entry.vm.log_path,
                )?;
                entry.process = Some(process);

                // Configure the VM
                crate::firecracker::configure_vm(&entry.vm)?;

                // Start the VM
                crate::firecracker::start_vm(&entry.vm)?;

                entry.vm.state = VmState::Running;
                Ok(entry.vm.clone())
            }
            VmState::Paused => {
                // Resume paused VM
                crate::firecracker::resume_vm(&entry.vm)?;
                entry.vm.state = VmState::Running;
                Ok(entry.vm.clone())
            }
            VmState::Running => Err(VmManagerError::InvalidState {
                current: VmState::Running,
                operation: "start".to_string(),
            }),
        }
    }

    pub async fn stop_vm(&self, vm_id: &str) -> Result<Vm, VmManagerError> {
        let mut vms = self.vms.write().await;

        let entry = vms
            .get_mut(vm_id)
            .ok_or_else(|| VmManagerError::VmNotFound(vm_id.to_string()))?;

        match entry.vm.state {
            VmState::Running | VmState::Paused => {
                // Kill the Firecracker process
                if let Some(ref mut process) = entry.process {
                    let _ = process.kill();
                }
                entry.process = None;
                entry.vm.state = VmState::Stopped;
                Ok(entry.vm.clone())
            }
            _ => Err(VmManagerError::InvalidState {
                current: entry.vm.state.clone(),
                operation: "stop".to_string(),
            }),
        }
    }

    pub async fn pause_vm(&self, vm_id: &str) -> Result<Vm, VmManagerError> {
        let mut vms = self.vms.write().await;

        let entry = vms
            .get_mut(vm_id)
            .ok_or_else(|| VmManagerError::VmNotFound(vm_id.to_string()))?;

        if entry.vm.state != VmState::Running {
            return Err(VmManagerError::InvalidState {
                current: entry.vm.state.clone(),
                operation: "pause".to_string(),
            });
        }

        crate::firecracker::pause_vm(&entry.vm)?;
        entry.vm.state = VmState::Paused;
        Ok(entry.vm.clone())
    }

    pub async fn get_vm(&self, vm_id: &str) -> Result<Vm, VmManagerError> {
        let vms = self.vms.read().await;
        vms.get(vm_id)
            .map(|entry| entry.vm.clone())
            .ok_or_else(|| VmManagerError::VmNotFound(vm_id.to_string()))
    }

    pub async fn list_vms(&self) -> Vec<Vm> {
        let vms = self.vms.read().await;
        vms.values().map(|entry| entry.vm.clone()).collect()
    }

    pub async fn delete_vm(&self, vm_id: &str) -> Result<(), VmManagerError> {
        let mut vms = self.vms.write().await;

        let entry = vms
            .get_mut(vm_id)
            .ok_or_else(|| VmManagerError::VmNotFound(vm_id.to_string()))?;

        // Stop the VM if running
        if let Some(ref mut process) = entry.process {
            let _ = process.kill();
        }

        vms.remove(vm_id);
        Ok(())
    }
}

impl Default for VmManager {
    fn default() -> Self {
        Self {
            vms: RwLock::new(HashMap::new()),
        }
    }
}
