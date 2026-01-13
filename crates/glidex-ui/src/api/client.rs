use crate::types::{ApiError, CreateVmRequest, HealthResponse, VmResponse};

/// Get the API base URL.
/// - In SSR (server-side): call the control plane directly
/// - In browser (WASM): use relative URLs through the UI server proxy
fn get_api_base_url() -> String {
    #[cfg(feature = "hydrate")]
    {
        // In browser, use the current origin + /api path (proxied through UI server)
        web_sys::window()
            .and_then(|w| w.location().origin().ok())
            .map(|origin| format!("{}/api", origin))
            .unwrap_or_else(|| "/api".to_string())
    }
    #[cfg(not(feature = "hydrate"))]
    {
        // In SSR, call the control plane directly
        "http://localhost:8080".to_string()
    }
}

pub async fn health_check() -> Result<HealthResponse, String> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/health", get_api_base_url()))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if resp.status().is_success() {
        resp.json().await.map_err(|e| e.to_string())
    } else {
        Err("Health check failed".to_string())
    }
}

pub async fn list_vms() -> Result<Vec<VmResponse>, String> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/vms", get_api_base_url()))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if resp.status().is_success() {
        resp.json().await.map_err(|e| e.to_string())
    } else {
        let error: ApiError = resp.json().await.map_err(|e| e.to_string())?;
        Err(format!("{}: {}", error.error, error.message))
    }
}

pub async fn get_vm(id: &str) -> Result<VmResponse, String> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/vms/{}", get_api_base_url(), id))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if resp.status().is_success() {
        resp.json().await.map_err(|e| e.to_string())
    } else if resp.status() == reqwest::StatusCode::NOT_FOUND {
        Err("VM not found".to_string())
    } else {
        let error: ApiError = resp.json().await.map_err(|e| e.to_string())?;
        Err(format!("{}: {}", error.error, error.message))
    }
}

pub async fn create_vm(request: CreateVmRequest) -> Result<VmResponse, String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/vms", get_api_base_url()))
        .json(&request)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if resp.status().is_success() {
        resp.json().await.map_err(|e| e.to_string())
    } else {
        let error: ApiError = resp.json().await.map_err(|e| e.to_string())?;
        Err(format!("{}: {}", error.error, error.message))
    }
}

pub async fn start_vm(id: &str) -> Result<VmResponse, String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/vms/{}/start", get_api_base_url(), id))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if resp.status().is_success() {
        resp.json().await.map_err(|e| e.to_string())
    } else {
        let error: ApiError = resp.json().await.map_err(|e| e.to_string())?;
        Err(format!("{}: {}", error.error, error.message))
    }
}

pub async fn stop_vm(id: &str) -> Result<VmResponse, String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/vms/{}/stop", get_api_base_url(), id))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if resp.status().is_success() {
        resp.json().await.map_err(|e| e.to_string())
    } else {
        let error: ApiError = resp.json().await.map_err(|e| e.to_string())?;
        Err(format!("{}: {}", error.error, error.message))
    }
}

pub async fn pause_vm(id: &str) -> Result<VmResponse, String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/vms/{}/pause", get_api_base_url(), id))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if resp.status().is_success() {
        resp.json().await.map_err(|e| e.to_string())
    } else {
        let error: ApiError = resp.json().await.map_err(|e| e.to_string())?;
        Err(format!("{}: {}", error.error, error.message))
    }
}

pub async fn delete_vm(id: &str) -> Result<(), String> {
    let client = reqwest::Client::new();
    let resp = client
        .delete(format!("{}/vms/{}", get_api_base_url(), id))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if resp.status().is_success() || resp.status() == reqwest::StatusCode::NO_CONTENT {
        Ok(())
    } else {
        let error: ApiError = resp.json().await.map_err(|e| e.to_string())?;
        Err(format!("{}: {}", error.error, error.message))
    }
}
