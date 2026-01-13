# GlideX Web UI

A Leptos-based web interface for managing Firecracker VMs through the GlideX control plane API.

## Features

- **Dashboard**: View all VMs in a responsive grid layout
- **VM Management**: Create, start, stop, pause, and delete VMs
- **Real-time Status**: Color-coded state indicators (green=running, red=stopped, yellow=paused, blue=created)
- **Health Monitoring**: API health status indicator in the header
- **VM Details**: Detailed view with console socket and log paths

## Prerequisites

- Rust (latest stable)
- [cargo-leptos](https://github.com/leptos-rs/cargo-leptos) for building
- Node.js and npm (for Tailwind CSS)
- GlideX control plane running on `http://localhost:8080`

## Installation

### 1. Install cargo-leptos

```bash
cargo install cargo-leptos
```

### 2. Install Tailwind CSS dependencies

```bash
cd crates/glidex-ui
npm install
```

### 3. Build Tailwind CSS

```bash
npm run css:build
```

## Running

### Development mode (with hot reload)

First, ensure the control plane is running:

```bash
cargo run --bin glidex-control-plane
```

Then start the UI in development mode:

```bash
cd crates/glidex-ui
cargo leptos watch
```

The UI will be available at **http://localhost:3000**

### Production build

```bash
cd crates/glidex-ui
cargo leptos build --release
```

The compiled binary will be in `target/release/glidex-ui`.

## Configuration

The UI connects to the control plane API at `http://localhost:8080` by default. This is configured in `src/api/client.rs`.

### Leptos Configuration

Build settings are defined in `Cargo.toml` under `[package.metadata.leptos]`:

| Setting | Default | Description |
|---------|---------|-------------|
| `site-addr` | `0.0.0.0:3000` | UI server address |
| `reload-port` | `3001` | Hot reload WebSocket port |
| `style-file` | `style/main.css` | Tailwind CSS entry point |
| `assets-dir` | `public` | Static assets directory |

## Project Structure

```
glidex-ui/
├── Cargo.toml              # Rust dependencies and Leptos config
├── package.json            # npm scripts for Tailwind
├── tailwind.config.js      # Tailwind configuration
├── style/
│   └── main.css            # Tailwind CSS entry point
├── public/                 # Static assets
└── src/
    ├── lib.rs              # Library entry (WASM hydration)
    ├── main.rs             # SSR server entry point
    ├── app.rs              # Root App component and routing
    ├── types.rs            # API types (VmState, VmResponse, etc.)
    ├── api/
    │   ├── mod.rs
    │   └── client.rs       # HTTP client for control plane API
    ├── components/
    │   ├── mod.rs
    │   ├── header.rs       # Navigation header with health indicator
    │   ├── vm_card.rs      # VM status card
    │   ├── vm_actions.rs   # Start/Stop/Pause/Delete buttons
    │   ├── create_vm_form.rs
    │   ├── modal.rs
    │   └── loading.rs
    └── pages/
        ├── mod.rs
        ├── dashboard.rs    # Main VM list view
        ├── vm_detail.rs    # Individual VM details
        └── not_found.rs    # 404 page
```

## Routes

| Path | Component | Description |
|------|-----------|-------------|
| `/` | Dashboard | Main view with VM grid and create button |
| `/vms/:id` | VmDetail | Detailed view of a specific VM |

## API Integration

The UI communicates with these control plane endpoints:

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/health` | Health check |
| GET | `/vms` | List all VMs |
| POST | `/vms` | Create a new VM |
| GET | `/vms/{id}` | Get VM details |
| DELETE | `/vms/{id}` | Delete a VM |
| POST | `/vms/{id}/start` | Start a VM |
| POST | `/vms/{id}/stop` | Stop a VM |
| POST | `/vms/{id}/pause` | Pause a VM |

## Styling

The UI uses Tailwind CSS with custom component classes defined in `style/main.css`:

- `.btn`, `.btn-primary`, `.btn-secondary`, `.btn-danger` - Button styles
- `.card` - Card container
- `.input` - Form input styling

To watch for CSS changes during development:

```bash
npm run css:watch
```

## Architecture

- **Leptos 0.7**: Rust web framework with fine-grained reactivity
- **SSR + Hydration**: Server-side rendering for fast initial load, client-side hydration for interactivity
- **Axum**: HTTP server for SSR
- **Tailwind CSS**: Utility-first CSS framework
