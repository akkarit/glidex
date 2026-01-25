use crate::hypervisor::{create_backend, Hypervisor, HypervisorError, HypervisorProcess, HypervisorType};
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
    HypervisorError(HypervisorError),
    PersistenceError(String),
    HypervisorNotAvailable(HypervisorType),
}

impl std::fmt::Display for VmManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VmManagerError::VmNotFound(id) => write!(f, "VM not found: {}", id),
            VmManagerError::VmAlreadyExists(name) => write!(f, "VM already exists: {}", name),
            VmManagerError::InvalidState { current, operation } => {
                write!(f, "Invalid state {:?} for operation: {}", current, operation)
            }
            VmManagerError::HypervisorError(e) => write!(f, "Hypervisor error: {}", e),
            VmManagerError::PersistenceError(e) => write!(f, "Persistence error: {}", e),
            VmManagerError::HypervisorNotAvailable(h) => {
                write!(f, "Hypervisor not available: {:?}", h)
            }
        }
    }
}

impl From<HypervisorError> for VmManagerError {
    fn from(e: HypervisorError) -> Self {
        VmManagerError::HypervisorError(e)
    }
}

impl From<PersistenceError> for VmManagerError {
    fn from(e: PersistenceError) -> Self {
        VmManagerError::PersistenceError(e.to_string())
    }
}

struct VmEntry {
    vm: Vm,
    process: Option<Box<dyn HypervisorProcess>>,
}

pub struct VmManager {
    vms: RwLock<HashMap<String, VmEntry>>,
    store: VmStore,
    backends: HashMap<HypervisorType, Box<dyn Hypervisor>>,
}

impl VmManager {
    /// Create a new VmManager with persistence at the default location
    pub fn new() -> Result<Arc<Self>, VmManagerError> {
        Self::with_db_path(Self::default_db_path())
    }

    /// Create a new VmManager with persistence at a custom path
    pub fn with_db_path(db_path: PathBuf) -> Result<Arc<Self>, VmManagerError> {
        let store = VmStore::open(&db_path)?;

        // Initialize hypervisor backends
        let mut backends: HashMap<HypervisorType, Box<dyn Hypervisor>> = HashMap::new();
        backends.insert(HypervisorType::Firecracker, create_backend(HypervisorType::Firecracker));
        backends.insert(HypervisorType::CloudHypervisor, create_backend(HypervisorType::CloudHypervisor));

        Ok(Arc::new(Self {
            vms: RwLock::new(HashMap::new()),
            store,
            backends,
        }))
    }

    /// Get the backend for a hypervisor type
    fn get_backend(&self, hypervisor: HypervisorType) -> Result<&dyn Hypervisor, VmManagerError> {
        self.backends
            .get(&hypervisor)
            .map(|b| b.as_ref())
            .ok_or(VmManagerError::HypervisorNotAvailable(hypervisor))
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
                // Check if the hypervisor process is still alive
                if self.is_hypervisor_alive(&vm.socket_path) {
                    // Process exists but we lost the handle - clean up and mark as stopped
                    self.cleanup_orphaned_vm(vm);
                }
                VmState::Stopped
            }
            VmState::Created | VmState::Stopped => vm.state.clone(),
        }
    }

    /// Check if a hypervisor process is still alive by probing its socket
    fn is_hypervisor_alive(&self, socket_path: &str) -> bool {
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
                // Get the appropriate backend for this VM's hypervisor
                let backend = self.get_backend(entry.vm.hypervisor)?;

                // Spawn hypervisor process with console socket and log file
                let process = backend.spawn(
                    &entry.vm.socket_path,
                    &entry.vm.console_socket_path,
                    &entry.vm.log_path,
                )?;

                // Configure the VM, cleanup process on failure
                if let Err(e) = process.configure(&entry.vm.config) {
                    let _ = process.kill();
                    return Err(e.into());
                }

                // Start the VM, cleanup process on failure
                if let Err(e) = process.start() {
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
                if let Some(ref process) = entry.process {
                    process.resume()?;
                } else {
                    return Err(VmManagerError::InvalidState {
                        current: VmState::Paused,
                        operation: "start (no process handle)".to_string(),
                    });
                }

                // Persist state change BEFORE updating in-memory state
                // If persist fails, pause again to maintain consistency
                if let Err(e) = self.store.update_state(vm_id, VmState::Running) {
                    if let Some(ref process) = entry.process {
                        let _ = process.pause();
                    }
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
                // Kill the hypervisor process (cannot be undone)
                if let Some(ref process) = entry.process {
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

        if let Some(ref process) = entry.process {
            process.pause()?;
        } else {
            return Err(VmManagerError::InvalidState {
                current: entry.vm.state.clone(),
                operation: "pause (no process handle)".to_string(),
            });
        }

        // Persist state change BEFORE updating in-memory state
        // If persist fails, resume the VM to maintain consistency
        if let Err(e) = self.store.update_state(vm_id, VmState::Paused) {
            if let Some(ref process) = entry.process {
                let _ = process.resume();
            }
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
        if let Some(ref process) = entry.process {
            let _ = process.kill();
        }

        // Delete from database BEFORE removing from memory
        self.store.delete(vm_id)?;

        vms.remove(vm_id);
        Ok(())
    }

    /// Shutdown all running VMs. Called during control-plane termination.
    pub async fn shutdown(&self) {
        let mut vms = self.vms.write().await;
        let mut stopped_count = 0;

        for (vm_id, entry) in vms.iter_mut() {
            if let Some(ref process) = entry.process {
                tracing::info!("Stopping VM {} ({})...", entry.vm.name, vm_id);
                let _ = process.kill();
                stopped_count += 1;

                // Update state in DB - log warning if fails
                if let Err(e) = self.store.update_state(vm_id, VmState::Stopped) {
                    tracing::warn!(
                        "Failed to persist VM {} state change to Stopped: {}",
                        vm_id,
                        e
                    );
                }
            }
            entry.process = None;
            entry.vm.state = VmState::Stopped;
        }

        if stopped_count > 0 {
            tracing::info!("Stopped {} running VM(s)", stopped_count);
        }
    }
}
