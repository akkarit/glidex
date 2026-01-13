use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use std::sync::Arc;

use crate::models::{ApiError, CreateVmRequest, VmConfig, VmResponse, VmState};
use crate::state::{VmManager, VmManagerError};
use serde::Serialize;

pub type AppState = Arc<VmManager>;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/vms", get(list_vms))
        .route("/vms", post(create_vm))
        .route("/vms/{id}", get(get_vm))
        .route("/vms/{id}", delete(delete_vm))
        .route("/vms/{id}/start", post(start_vm))
        .route("/vms/{id}/stop", post(stop_vm))
        .route("/vms/{id}/pause", post(pause_vm))
        .route("/vms/{id}/console", get(get_console_info))
        .route("/health", get(health_check))
        .with_state(state)
}

async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok" }))
}

async fn list_vms(State(manager): State<AppState>) -> impl IntoResponse {
    let vms = manager.list_vms().await;
    let response: Vec<VmResponse> = vms.iter().map(VmResponse::from).collect();
    Json(response)
}

async fn create_vm(
    State(manager): State<AppState>,
    Json(request): Json<CreateVmRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let name = request.name.clone();
    let config = VmConfig::from(request);

    match manager.create_vm(name, config).await {
        Ok(vm) => Ok((StatusCode::CREATED, Json(VmResponse::from(&vm)))),
        Err(e) => Err(error_to_response(e)),
    }
}

async fn get_vm(
    State(manager): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    match manager.get_vm(&id).await {
        Ok(vm) => Ok(Json(VmResponse::from(&vm))),
        Err(e) => Err(error_to_response(e)),
    }
}

async fn delete_vm(
    State(manager): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    match manager.delete_vm(&id).await {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(e) => Err(error_to_response(e)),
    }
}

async fn start_vm(
    State(manager): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    match manager.start_vm(&id).await {
        Ok(vm) => Ok(Json(VmResponse::from(&vm))),
        Err(e) => Err(error_to_response(e)),
    }
}

async fn stop_vm(
    State(manager): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    match manager.stop_vm(&id).await {
        Ok(vm) => Ok(Json(VmResponse::from(&vm))),
        Err(e) => Err(error_to_response(e)),
    }
}

async fn pause_vm(
    State(manager): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    match manager.pause_vm(&id).await {
        Ok(vm) => Ok(Json(VmResponse::from(&vm))),
        Err(e) => Err(error_to_response(e)),
    }
}

#[derive(Debug, Serialize)]
struct ConsoleInfo {
    vm_id: String,
    console_socket_path: String,
    log_path: String,
    available: bool,
}

async fn get_console_info(
    State(manager): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    match manager.get_vm(&id).await {
        Ok(vm) => {
            let available = vm.state == VmState::Running;
            Ok(Json(ConsoleInfo {
                vm_id: vm.id,
                console_socket_path: vm.console_socket_path,
                log_path: vm.log_path,
                available,
            }))
        }
        Err(e) => Err(error_to_response(e)),
    }
}

fn error_to_response(error: VmManagerError) -> (StatusCode, Json<ApiError>) {
    match &error {
        VmManagerError::VmNotFound(_) => (
            StatusCode::NOT_FOUND,
            Json(ApiError::new("not_found", error.to_string())),
        ),
        VmManagerError::VmAlreadyExists(_) => (
            StatusCode::CONFLICT,
            Json(ApiError::new("conflict", error.to_string())),
        ),
        VmManagerError::InvalidState { .. } => (
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("invalid_state", error.to_string())),
        ),
        VmManagerError::FirecrackerError(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("firecracker_error", error.to_string())),
        ),
        VmManagerError::PersistenceError(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("persistence_error", error.to_string())),
        ),
    }
}
