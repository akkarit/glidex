use serde::Serialize;
use std::fs;
use std::path::Path;

const PCI_DEVICES_PATH: &str = "/sys/bus/pci/devices";

#[derive(Debug, Clone, Serialize)]
pub struct PciDeviceInfo {
    pub address: String,
    pub vendor_id: String,
    pub device_id: String,
    pub class_id: String,
    pub driver: Option<String>,
    pub iommu_group: Option<String>,
    pub sysfs_path: String,
}

/// Read a sysfs attribute file, returning trimmed contents.
fn read_sysfs_attr(device_path: &Path, attr: &str) -> Option<String> {
    fs::read_to_string(device_path.join(attr))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Get the driver name for a PCI device by reading the driver symlink.
fn get_driver(device_path: &Path) -> Option<String> {
    let driver_link = device_path.join("driver");
    fs::read_link(&driver_link)
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
}

/// Get the IOMMU group number for a PCI device.
fn get_iommu_group(device_path: &Path) -> Option<String> {
    let iommu_link = device_path.join("iommu_group");
    fs::read_link(&iommu_link)
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
}

/// Scan all PCI devices from sysfs and return their information.
pub fn scan_pci_devices() -> Vec<PciDeviceInfo> {
    let pci_path = Path::new(PCI_DEVICES_PATH);
    if !pci_path.exists() {
        return Vec::new();
    }

    let mut devices = Vec::new();

    let entries = match fs::read_dir(pci_path) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    for entry in entries.flatten() {
        let device_path = entry.path();
        let address = match entry.file_name().to_str() {
            Some(name) => name.to_string(),
            None => continue,
        };

        let vendor_id = read_sysfs_attr(&device_path, "vendor").unwrap_or_default();
        let device_id = read_sysfs_attr(&device_path, "device").unwrap_or_default();
        let class_id = read_sysfs_attr(&device_path, "class").unwrap_or_default();
        let driver = get_driver(&device_path);
        let iommu_group = get_iommu_group(&device_path);

        devices.push(PciDeviceInfo {
            address: address.clone(),
            vendor_id,
            device_id,
            class_id,
            driver,
            iommu_group,
            sysfs_path: format!("{}/{}", PCI_DEVICES_PATH, address),
        });
    }

    devices.sort_by(|a, b| a.address.cmp(&b.address));
    devices
}
