use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

use glidex_control_plane::api::create_router;
use glidex_control_plane::state::VmManager;

/// Helper to create a test app instance
fn create_test_app() -> axum::Router {
    let vm_manager = VmManager::new();
    create_router(vm_manager)
}

/// Helper to extract JSON body from response
async fn body_to_json(body: Body) -> Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

// ============================================================================
// Health Check Tests
// ============================================================================

#[tokio::test]
async fn test_health_check() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = body_to_json(response.into_body()).await;
    assert_eq!(body["status"], "ok");
}

// ============================================================================
// VM List Tests
// ============================================================================

#[tokio::test]
async fn test_list_vms_empty() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/vms")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = body_to_json(response.into_body()).await;
    assert!(body.is_array());
    assert_eq!(body.as_array().unwrap().len(), 0);
}

// ============================================================================
// VM Create Tests
// ============================================================================

#[tokio::test]
async fn test_create_vm_success() {
    let app = create_test_app();

    let create_request = json!({
        "name": "test-vm",
        "vcpu_count": 2,
        "mem_size_mib": 512,
        "kernel_image_path": "/path/to/kernel",
        "rootfs_path": "/path/to/rootfs.ext4"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/vms")
                .header("content-type", "application/json")
                .body(Body::from(create_request.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = body_to_json(response.into_body()).await;
    assert_eq!(body["name"], "test-vm");
    assert_eq!(body["vcpu_count"], 2);
    assert_eq!(body["mem_size_mib"], 512);
    assert_eq!(body["state"], "created");
    assert!(body["id"].is_string());
}

#[tokio::test]
async fn test_create_vm_with_optional_fields() {
    let app = create_test_app();

    let create_request = json!({
        "name": "test-vm-full",
        "vcpu_count": 4,
        "mem_size_mib": 1024,
        "kernel_image_path": "/path/to/kernel",
        "rootfs_path": "/path/to/rootfs.ext4",
        "kernel_args": "console=ttyS0 reboot=k panic=1"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/vms")
                .header("content-type", "application/json")
                .body(Body::from(create_request.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = body_to_json(response.into_body()).await;
    assert_eq!(body["name"], "test-vm-full");
    assert_eq!(body["vcpu_count"], 4);
    assert_eq!(body["mem_size_mib"], 1024);
}

#[tokio::test]
async fn test_create_vm_duplicate_name() {
    let vm_manager = VmManager::new();
    let app = create_router(vm_manager);

    let create_request = json!({
        "name": "duplicate-vm",
        "vcpu_count": 1,
        "mem_size_mib": 256,
        "kernel_image_path": "/path/to/kernel",
        "rootfs_path": "/path/to/rootfs.ext4"
    });

    // Create first VM
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/vms")
                .header("content-type", "application/json")
                .body(Body::from(create_request.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    // Try to create VM with same name
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/vms")
                .header("content-type", "application/json")
                .body(Body::from(create_request.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);

    let body = body_to_json(response.into_body()).await;
    assert_eq!(body["error"], "conflict");
}

#[tokio::test]
async fn test_create_vm_invalid_json() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/vms")
                .header("content-type", "application/json")
                .body(Body::from("invalid json"))
                .unwrap(),
        )
        .await
        .unwrap();

    // Axum returns 400 Bad Request for malformed JSON
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_create_vm_missing_required_fields() {
    let app = create_test_app();

    let create_request = json!({
        "name": "test-vm"
        // missing vcpu_count, mem_size_mib, kernel_image_path
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/vms")
                .header("content-type", "application/json")
                .body(Body::from(create_request.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

// ============================================================================
// VM Get Tests
// ============================================================================

#[tokio::test]
async fn test_get_vm_success() {
    let vm_manager = VmManager::new();
    let app = create_router(vm_manager);

    // Create a VM first
    let create_request = json!({
        "name": "get-test-vm",
        "vcpu_count": 2,
        "mem_size_mib": 512,
        "kernel_image_path": "/path/to/kernel",
        "rootfs_path": "/path/to/rootfs.ext4"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/vms")
                .header("content-type", "application/json")
                .body(Body::from(create_request.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let created_vm = body_to_json(response.into_body()).await;
    let vm_id = created_vm["id"].as_str().unwrap();

    // Get the VM
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/vms/{}", vm_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = body_to_json(response.into_body()).await;
    assert_eq!(body["id"], vm_id);
    assert_eq!(body["name"], "get-test-vm");
}

#[tokio::test]
async fn test_get_vm_not_found() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/vms/nonexistent-id")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let body = body_to_json(response.into_body()).await;
    assert_eq!(body["error"], "not_found");
}

// ============================================================================
// VM Delete Tests
// ============================================================================

#[tokio::test]
async fn test_delete_vm_success() {
    let vm_manager = VmManager::new();
    let app = create_router(vm_manager);

    // Create a VM first
    let create_request = json!({
        "name": "delete-test-vm",
        "vcpu_count": 1,
        "mem_size_mib": 256,
        "kernel_image_path": "/path/to/kernel",
        "rootfs_path": "/path/to/rootfs.ext4"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/vms")
                .header("content-type", "application/json")
                .body(Body::from(create_request.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let created_vm = body_to_json(response.into_body()).await;
    let vm_id = created_vm["id"].as_str().unwrap();

    // Delete the VM
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/vms/{}", vm_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify VM is gone
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/vms/{}", vm_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_vm_not_found() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/vms/nonexistent-id")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ============================================================================
// VM List After Operations Tests
// ============================================================================

#[tokio::test]
async fn test_list_vms_after_create() {
    let vm_manager = VmManager::new();
    let app = create_router(vm_manager);

    // Create two VMs
    for name in ["vm-1", "vm-2"] {
        let create_request = json!({
            "name": name,
            "vcpu_count": 1,
            "mem_size_mib": 256,
            "kernel_image_path": "/path/to/kernel",
            "rootfs_path": "/path/to/rootfs.ext4"
        });

        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/vms")
                    .header("content-type", "application/json")
                    .body(Body::from(create_request.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // List VMs
    let response = app
        .oneshot(
            Request::builder()
                .uri("/vms")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = body_to_json(response.into_body()).await;
    assert!(body.is_array());
    assert_eq!(body.as_array().unwrap().len(), 2);
}

// ============================================================================
// VM Lifecycle Tests (without actual Firecracker)
// ============================================================================

#[tokio::test]
async fn test_start_vm_not_found() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/vms/nonexistent-id/start")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_stop_vm_not_found() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/vms/nonexistent-id/stop")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_pause_vm_not_found() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/vms/nonexistent-id/pause")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_stop_vm_invalid_state() {
    let vm_manager = VmManager::new();
    let app = create_router(vm_manager);

    // Create a VM (state: created)
    let create_request = json!({
        "name": "stop-test-vm",
        "vcpu_count": 1,
        "mem_size_mib": 256,
        "kernel_image_path": "/path/to/kernel",
        "rootfs_path": "/path/to/rootfs.ext4"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/vms")
                .header("content-type", "application/json")
                .body(Body::from(create_request.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let created_vm = body_to_json(response.into_body()).await;
    let vm_id = created_vm["id"].as_str().unwrap();

    // Try to stop a VM that is not running (should fail)
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/vms/{}/stop", vm_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = body_to_json(response.into_body()).await;
    assert_eq!(body["error"], "invalid_state");
}

#[tokio::test]
async fn test_pause_vm_invalid_state() {
    let vm_manager = VmManager::new();
    let app = create_router(vm_manager);

    // Create a VM (state: created)
    let create_request = json!({
        "name": "pause-test-vm",
        "vcpu_count": 1,
        "mem_size_mib": 256,
        "kernel_image_path": "/path/to/kernel",
        "rootfs_path": "/path/to/rootfs.ext4"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/vms")
                .header("content-type", "application/json")
                .body(Body::from(create_request.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let created_vm = body_to_json(response.into_body()).await;
    let vm_id = created_vm["id"].as_str().unwrap();

    // Try to pause a VM that is not running (should fail)
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/vms/{}/pause", vm_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = body_to_json(response.into_body()).await;
    assert_eq!(body["error"], "invalid_state");
}

// ============================================================================
// Console Endpoint Tests
// ============================================================================

#[tokio::test]
async fn test_get_console_info_not_found() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/vms/nonexistent-id/console")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_console_info_vm_not_running() {
    let vm_manager = VmManager::new();
    let app = create_router(vm_manager);

    // Create a VM
    let create_request = json!({
        "name": "console-test-vm",
        "vcpu_count": 1,
        "mem_size_mib": 256,
        "kernel_image_path": "/path/to/kernel",
        "rootfs_path": "/path/to/rootfs.ext4"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/vms")
                .header("content-type", "application/json")
                .body(Body::from(create_request.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let created_vm = body_to_json(response.into_body()).await;
    let vm_id = created_vm["id"].as_str().unwrap();

    // Get console info (VM not running)
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/vms/{}/console", vm_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = body_to_json(response.into_body()).await;
    assert_eq!(body["vm_id"], vm_id);
    assert_eq!(body["available"], false);
    // console_socket_path and log_path should be present
    assert!(body["console_socket_path"].is_string());
    assert!(body["log_path"].is_string());
}
