#[cfg(feature = "ssr")]
mod api_proxy {
    use axum::{
        extract::Path,
        http::{Method, StatusCode},
        response::{IntoResponse, Response},
        routing::{get, post},
        Router,
    };

    const CONTROL_PLANE_URL: &str = "http://localhost:8080";

    async fn proxy_request(method: Method, path: &str, body: Option<String>) -> Response {
        let client = reqwest::Client::new();
        let url = format!("{}{}", CONTROL_PLANE_URL, path);

        let mut request = match method {
            Method::GET => client.get(&url),
            Method::POST => client.post(&url),
            Method::DELETE => client.delete(&url),
            _ => {
                return (StatusCode::METHOD_NOT_ALLOWED, "Method not allowed").into_response();
            }
        };

        if let Some(body) = body {
            request = request
                .header("Content-Type", "application/json")
                .body(body);
        }

        match request.send().await {
            Ok(resp) => {
                let status = StatusCode::from_u16(resp.status().as_u16())
                    .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
                let body = resp.text().await.unwrap_or_default();
                (
                    status,
                    [("Content-Type", "application/json")],
                    body,
                )
                    .into_response()
            }
            Err(e) => {
                tracing::error!("Proxy request failed: {}", e);
                (
                    StatusCode::BAD_GATEWAY,
                    format!("Failed to connect to control plane: {}", e),
                )
                    .into_response()
            }
        }
    }

    async fn health() -> Response {
        proxy_request(Method::GET, "/health", None).await
    }

    async fn list_vms() -> Response {
        proxy_request(Method::GET, "/vms", None).await
    }

    async fn create_vm(body: String) -> Response {
        proxy_request(Method::POST, "/vms", Some(body)).await
    }

    async fn get_vm(Path(id): Path<String>) -> Response {
        proxy_request(Method::GET, &format!("/vms/{}", id), None).await
    }

    async fn delete_vm(Path(id): Path<String>) -> Response {
        proxy_request(Method::DELETE, &format!("/vms/{}", id), None).await
    }

    async fn start_vm(Path(id): Path<String>) -> Response {
        proxy_request(Method::POST, &format!("/vms/{}/start", id), None).await
    }

    async fn stop_vm(Path(id): Path<String>) -> Response {
        proxy_request(Method::POST, &format!("/vms/{}/stop", id), None).await
    }

    async fn pause_vm(Path(id): Path<String>) -> Response {
        proxy_request(Method::POST, &format!("/vms/{}/pause", id), None).await
    }

    async fn get_console(Path(id): Path<String>) -> Response {
        proxy_request(Method::GET, &format!("/vms/{}/console", id), None).await
    }

    pub fn router() -> Router {
        Router::new()
            .route("/api/health", get(health))
            .route("/api/vms", get(list_vms).post(create_vm))
            .route("/api/vms/{id}", get(get_vm).delete(delete_vm))
            .route("/api/vms/{id}/start", post(start_vm))
            .route("/api/vms/{id}/stop", post(stop_vm))
            .route("/api/vms/{id}/pause", post(pause_vm))
            .route("/api/vms/{id}/console", get(get_console))
    }
}

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use axum::Router;
    use leptos::prelude::*;
    use leptos_axum::{generate_route_list, LeptosRoutes};
    use glidex_ui::app::{shell, App};

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let conf = get_configuration(None).expect("Failed to get Leptos configuration");
    let leptos_options = conf.leptos_options;
    let addr = leptos_options.site_addr;
    let routes = generate_route_list(App);

    // Build the Leptos app router
    let leptos_router = Router::new()
        .leptos_routes(&leptos_options, routes, {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        .fallback(leptos_axum::file_and_error_handler(shell))
        .with_state(leptos_options);

    // Combine API proxy routes with Leptos routes
    // API routes are checked first due to Router::merge precedence
    let app = api_proxy::router().merge(leptos_router);

    tracing::info!("GlideX UI listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.expect("Failed to bind");
    axum::serve(listener, app.into_make_service())
        .await
        .expect("Server error");
}

#[cfg(not(feature = "ssr"))]
fn main() {
    // This is used for the WASM build - the actual entry point is the hydrate function in lib.rs
}
