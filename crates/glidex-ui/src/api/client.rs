use crate::types::{ApiError, CreateVmRequest, HealthResponse, VmResponse};

const API_BASE_URL: &str = "http://localhost:8080";

pub async fn health_check() -> Result<HealthResponse, String> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/health", API_BASE_URL))
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
        .get(format!("{}/vms", API_BASE_URL))
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
        .get(format!("{}/vms/{}", API_BASE_URL, id))
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
        .post(format!("{}/vms", API_BASE_URL))
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
        .post(format!("{}/vms/{}/start", API_BASE_URL, id))
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
        .post(format!("{}/vms/{}/stop", API_BASE_URL, id))
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
        .post(format!("{}/vms/{}/pause", API_BASE_URL, id))
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
        .delete(format!("{}/vms/{}", API_BASE_URL, id))
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
