use crate::firecracker::{FirecrackerError, FirecrackerProcess};
use crate::models::{Vm, VmConfig, VmState};
use crate::persistence::{PersistenceError, VmStore};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug)]
pub enum VmManagerError {
    VmNotFound(String),
    VmAlreadyExists(String),
    InvalidState { current: VmState, operation: String },
    FirecrackerError(FirecrackerError),
    PersistenceError(String),
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
            VmManagerError::PersistenceError(e) => write!(f, "Persistence error: {}", e),
        }
    }
}

impl From<FirecrackerError> for VmManagerError {
    fn from(e: FirecrackerError) -> Self {
        VmManagerError::FirecrackerError(e)
    }
}

impl From<PersistenceError> for VmManagerError {
    fn from(e: PersistenceError) -> Self {
        VmManagerError::PersistenceError(e.to_string())
    }
}

struct VmEntry {
    vm: Vm,
    process: Option<FirecrackerProcess>,
}

pub struct VmManager {
    vms: RwLock<HashMap<String, VmEntry>>,
    store: VmStore,
}

impl VmManager {
    /// Create a new VmManager with persistence at the default location
    pub fn new() -> Result<Arc<Self>, VmManagerError> {
        Self::with_db_path(Self::default_db_path())
    }

    /// Create a new VmManager with persistence at a custom path
    pub fn with_db_path(db_path: PathBuf) -> Result<Arc<Self>, VmManagerError> {
        let store = VmStore::open(&db_path)?;
        Ok(Arc::new(Self {
            vms: RwLock::new(HashMap::new()),
            store,
        }))
    }

    /// Get the default database path (~/.glidex/glidex.db)
    fn default_db_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".glidex")
            .join("glidex.db")
    }

    /// Initialize VmManager by loading persisted VMs and reconciling state
    pub async fn initialize(&self) -> Result<(), VmManagerError> {
        let persisted_vms = self.store.load_all()?;
        let mut vms = self.vms.write().await;

        for mut vm in persisted_vms {
            // Reconcile state: VMs that were Running/Paused are now orphaned
            let reconciled_state = self.reconcile_vm_state(&vm);

            if vm.state != reconciled_state {
                vm.state = reconciled_state;
                // Update DB with reconciled state
                self.store.save(&vm)?;
            }

            vms.insert(
                vm.id.clone(),
                VmEntry {
                    vm,
                    process: None, // Process handles cannot be restored
                },
            );
        }

        Ok(())
    }

    /// Reconcile VM state after restart
    fn reconcile_vm_state(&self, vm: &Vm) -> VmState {
        match vm.state {
            VmState::Running | VmState::Paused => {
                // Check if the Firecracker process is still alive
                if self.is_firecracker_alive(&vm.socket_path) {
                    // Process exists but we lost the handle - clean up and mark as stopped
                    self.cleanup_orphaned_vm(vm);
                }
                VmState::Stopped
            }
            VmState::Created | VmState::Stopped => vm.state.clone(),
        }
    }

    /// Check if a Firecracker process is still alive by probing its socket
    fn is_firecracker_alive(&self, socket_path: &str) -> bool {
        std::path::Path::new(socket_path).exists()
    }

    /// Clean up resources from an orphaned VM
    fn cleanup_orphaned_vm(&self, vm: &Vm) {
        // Remove socket files
        let _ = std::fs::remove_file(&vm.socket_path);
        let _ = std::fs::remove_file(&vm.console_socket_path);

        tracing::warn!(
            "Cleaned up orphaned VM resources for {} ({})",
            vm.name,
            vm.id
        );
    }

    pub async fn create_vm(&self, name: String, config: VmConfig) -> Result<Vm, VmManagerError> {
        let mut vms = self.vms.write().await;

        // Check if VM with same name exists
        if vms.values().any(|entry| entry.vm.name == name) {
            return Err(VmManagerError::VmAlreadyExists(name));
        }

        let vm = Vm::new(name, config);

        // Persist to database BEFORE adding to in-memory cache
        self.store.save(&vm)?;

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
                let mut process = FirecrackerProcess::spawn(
                    &entry.vm.socket_path,
                    &entry.vm.console_socket_path,
                    &entry.vm.log_path,
                )?;

                // Configure the VM, cleanup process on failure
                if let Err(e) = crate::firecracker::configure_vm(&entry.vm) {
                    let _ = process.kill();
                    return Err(e.into());
                }

                // Start the VM, cleanup process on failure
                if let Err(e) = crate::firecracker::start_vm(&entry.vm) {
                    let _ = process.kill();
                    return Err(e.into());
                }

                // Persist state change BEFORE updating in-memory state
                // If persist fails, kill the process to maintain consistency
                if let Err(e) = self.store.update_state(vm_id, VmState::Running) {
                    let _ = process.kill();
                    return Err(e.into());
                }

                entry.process = Some(process);
                entry.vm.state = VmState::Running;

                Ok(entry.vm.clone())
            }
            VmState::Paused => {
                // Resume paused VM
                crate::firecracker::resume_vm(&entry.vm)?;

                // Persist state change BEFORE updating in-memory state
                // If persist fails, pause again to maintain consistency
                if let Err(e) = self.store.update_state(vm_id, VmState::Running) {
                    let _ = crate::firecracker::pause_vm(&entry.vm);
                    return Err(e.into());
                }

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
                // Kill the Firecracker process (cannot be undone)
                if let Some(ref mut process) = entry.process {
                    let _ = process.kill();
                }
                entry.process = None;
                entry.vm.state = VmState::Stopped;

                // Persist state change - log warning if fails since operation already happened
                if let Err(e) = self.store.update_state(vm_id, VmState::Stopped) {
                    tracing::error!(
                        "Failed to persist VM {} state change to Stopped: {}. State will be reconciled on restart.",
                        vm_id, e
                    );
                }

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

        // Persist state change BEFORE updating in-memory state
        // If persist fails, resume the VM to maintain consistency
        if let Err(e) = self.store.update_state(vm_id, VmState::Paused) {
            let _ = crate::firecracker::resume_vm(&entry.vm);
            return Err(e.into());
        }

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

        // Delete from database BEFORE removing from memory
        self.store.delete(vm_id)?;

        vms.remove(vm_id);
        Ok(())
    }
}
