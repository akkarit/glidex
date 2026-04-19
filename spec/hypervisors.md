# Hypervisor abstraction

Source: `crates/glidex-control-plane/src/hypervisor/`.

## The traits

`Hypervisor` is a **factory** for running VMs. It has no instance
state of its own; it's stateless except for knowing what backend it
represents.

```rust
pub trait Hypervisor: Send + Sync {
    fn spawn(
        &self,
        socket_path: &str,          // hypervisor API / QMP socket
        console_socket_path: &str,  // client-facing console
        log_path: &str,             // append-only captured console
    ) -> Result<Box<dyn HypervisorProcess>, HypervisorError>;

    fn hypervisor_type(&self) -> HypervisorType;
    fn is_available(&self) -> bool;  // is the binary on PATH?
}
```

`HypervisorProcess` is the **running VM handle**. Every method is
`&self` — mutable state (child pid, console thread join handle,
atomic run flag) is behind interior-mutability primitives so the
handle is `Send + Sync`:

```rust
pub trait HypervisorProcess: Send + Sync {
    fn configure(&self, config: &VmConfig) -> Result<(), HypervisorError>;
    fn start(&self) -> Result<(), HypervisorError>;
    fn pause(&self) -> Result<(), HypervisorError>;
    fn resume(&self) -> Result<(), HypervisorError>;
    fn kill(&self) -> Result<(), HypervisorError>;

    fn add_device(&self, device_path: &str) -> Result<(), HypervisorError>;     // default: Unsupported
    fn remove_device(&self, device_path: &str) -> Result<(), HypervisorError>;  // default: Unsupported

    fn is_running(&self) -> bool;
    fn socket_path(&self) -> &str;
    fn console_socket_path(&self) -> &str;
    fn log_path(&self) -> &str;
}
```

### Trait contract

- `spawn` **may or may not** launch the actual hypervisor binary.
  Firecracker and Cloud-Hypervisor launch in `spawn` (their APIs are
  HTTP: you talk to them while they wait for config). QEMU launches
  in `configure` because QEMU needs the full config on its command
  line; see below.
- `configure` must be called exactly once, before `start`.
- After `kill`, the process handle is done. A fresh `spawn` +
  `configure` + `start` is required to bring the VM back.
- `add_device` / `remove_device` on an already-running VM are
  hot-plug operations and must go through the hypervisor's live
  management API. Backends that don't support it can leave the
  default impls, which return `Unsupported`.
- **Console listener lifetime** (see [console.md](console.md)): the
  console Unix socket listener must remain bound from `spawn` (or
  `configure`, for QEMU) until `kill`. It must not be dropped just
  because the guest died.

### Backend selection

`VmManager` maintains a `HashMap<HypervisorType, Box<dyn Hypervisor>>`
populated at startup with all three backends. On `start_vm`, it
looks the VM's `hypervisor` up in that map and delegates to it.
Backends whose binary is missing from `PATH` are still registered —
the error surfaces only at launch time.

## Firecracker

Source: `hypervisor/firecracker.rs`. API: Firecracker's own HTTP/JSON
control protocol over a Unix socket.

`spawn` immediately forks `firecracker --api-sock <sock>` with its
stdio attached to a PTY slave. The PTY master is handed to a proxy
thread that bridges the PTY to the client-facing console Unix socket
(`hypervisor/firecracker.rs::console_proxy_loop`) and tees everything
into the log file.

`configure` issues three HTTP `PUT`s on the API socket:

1. `/machine-config` — CPU count + memory.
2. `/boot-source` — kernel image path + boot args.
3. `/drives/rootfs` — rootfs file as the root drive.

`start` issues `/actions` with `{"action_type":"InstanceStart"}`;
`pause` / `resume` is a `PATCH /vm` with `{"state": …}`. There is no
hot-plug device support — `add_device`/`remove_device` fall back to
the trait's `Unsupported` default.

## Cloud-Hypervisor

Source: `hypervisor/cloud_hypervisor.rs`. API: CH's HTTP protocol
over a Unix socket. Message framing is hand-rolled to match the
upstream `api_client` format exactly (see `send_request` in that
file).

`spawn` runs `cloud-hypervisor --api-socket <sock>` with stdio muted.
The PTY-based proxy thread is *not* started in `spawn`; CH allocates
its own PTY when the VM boots. We discover that PTY path through
`vm.info` and only then start `start_console_proxy`, which opens the
PTY and bridges it to the console Unix socket.

`configure` does a single `PUT /vm.create` with a full config
payload (CPU, memory, kernel payload, disks, console/serial config,
any VFIO devices). Console mode is `"Pty"`, serial is `"Off"`.
`start` issues `PUT /vm.boot`, then polls `vm.info` to discover the
allocated console PTY, and starts the console proxy against it.

`pause` / `resume` / `kill` map directly to the corresponding CH API
endpoints. `add_device` / `remove_device` use CH's `/vm.add-device`
and `/vm.remove-device`, with a deterministic device id derived from
the sysfs BDF (`_vfio_0000_41_00_0`).

## QEMU

Source: `hypervisor/qemu.rs`. API: **QMP** (QEMU Machine Protocol)
over a Unix socket.

### Deferred launch

QEMU is fundamentally different from the other two: it takes *all*
its config on the command line. There is no runtime "configure" API
once it's running. Therefore:

- `QemuBackend::spawn` does **nothing but allocate the handle**. No
  `qemu-system-x86_64` process is started.
- `QemuInstance::configure(&config)` is where the process is
  actually launched, with `-S` so the guest is paused at reset.
- `start` then sends QMP `cont` to unfreeze it.

The command line (from `qemu.rs::launch`):

```
qemu-system-x86_64
  -enable-kvm
  -no-reboot
  -machine q35
  -m <mem>M
  -smp <vcpus>
  -kernel <kernel_image_path>
  -append "<kernel_args>"
  -drive file=<rootfs_path>,if=virtio,format=raw
  -qmp unix:<socket_path>,server,nowait
  -serial stdio
  -display none
  -S
  [-device vfio-pci,host=<bdf>,id=<_vfio_xxx> …]
```

Notes captured in code comments:

- We avoid `-nographic` because it implies `-serial mon:stdio` and
  collides with our explicit `-serial stdio`.
- We avoid `-cpu host` because it fails on hosts whose feature set
  isn't expressible.
- We use `server,nowait` (pre-6.0 syntax) because it's accepted by
  both old and new QEMU, unlike the newer `server=on,wait=off`.

### QMP client

`QmpClient` in the same file opens a *new* Unix connection per
command. Each connection:

1. Reads the server greeting line.
2. Sends `{"execute":"qmp_capabilities"}` and reads the `return`.
3. Sends the actual command JSON and waits for the first
   `return`/`error` line, skipping any asynchronous `event` lines
   in between.

Commands used:

| Operation | QMP command |
|---|---|
| `start` / `resume` | `cont` |
| `pause` | `stop` |
| `kill` | `quit` (best-effort; child is also killed) |
| `add_device` | `device_add` with `driver=vfio-pci`, `host=<bdf>`, `id=<deterministic>` |
| `remove_device` | `device_del` with `id=<deterministic>` |

### Launch health check

`launch` loops up to 5 seconds waiting for the QMP socket to appear
**and** respond with a greeting. It also calls `child_exit_status()`
each iteration: if QEMU has already exited, we read the captured
log (from the PTY proxy) and return a `ProcessStart` error whose
message embeds the tail of QEMU's stderr/stdout. This is what
surfaces misconfigured kernel paths, missing KVM, and other
launch-time errors.

### Default kernel args

`HypervisorType::Qemu::default_kernel_args` is
`"console=ttyS0 root=/dev/vda reboot=k panic=1"`. `root=/dev/vda`
(no partition number) matches the bare-ext4 sample rootfs produced
by `glidex-install`, which has no partition table.

## VFIO device identifiers

All three backends that support VFIO derive a stable *id* for a
device from the sysfs path. Given
`/sys/bus/pci/devices/0000:41:00.0` the id is `_vfio_0000_41_00_0`
(colons/dots replaced with underscores, `_vfio_` prefix). This id is:

- Used in the hypervisor's attach/detach calls so detach can refer
  to the same device that was attached.
- Not persisted. `Vm.config.vfio_devices` stores the sysfs path —
  the id is rederived when needed.
