use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{delete, get, patch, post};
use axum::Router;
use rust_embed::Embed;
use tokio::sync::broadcast;

use crate::api::{self, AppState, SharedState};
use crate::config;
use crate::registry::Registry;
use crate::scanner;
use crate::secrets::SecretStore;
use crate::ws::WsEvent;

#[derive(Embed)]
#[folder = "dashboard/"]
struct DashboardAssets;

pub async fn run(bind: String, port: u16) {
    let db_path = config::db_path();
    let registry = Arc::new(Registry::open(&db_path));
    let secrets = Arc::new(SecretStore::new(registry.clone()));
    let (tx, _rx) = broadcast::channel::<WsEvent>(256);

    let state: AppState = Arc::new(SharedState {
        registry: registry.clone(),
        tx: tx.clone(),
        secrets,
    });

    let app = Router::new()
        .route("/projects", get(api::get_projects))
        .route("/projects", post(api::add_project))
        .route("/projects/{id}/kill", post(api::kill_project))
        .route("/projects/{id}/restart", post(api::restart_project))
        .route("/projects/{id}/processes/{pid}/restart", post(api::restart_process))
        .route("/projects/{id}", patch(api::patch_project))
        .route("/projects/{id}", delete(api::delete_project))
        .route("/ports", get(api::get_ports))
        .route("/kill/{port}", post(api::kill_port))
        .route("/projects/{id}/env", get(api::get_project_env))
        .route("/projects/{id}/env/{key}", patch(api::update_project_env_key))
        .route("/secrets", get(api::list_secrets))
        .route("/secrets", post(api::set_secret))
        .route("/secrets/{key}", get(api::get_secret))
        .route("/secrets/{key}", delete(api::delete_secret))
        .route("/ws", get(ws_handler))
        .fallback(get(serve_dashboard))
        .with_state(state);

    let addr: SocketAddr = format!("{}:{}", bind, port)
        .parse()
        .expect("Invalid bind address");

    // Start scanner loop
    let scan_interval = config::Config::load().scan_interval_secs;
    tokio::spawn(scanner::run_loop(registry, tx, scan_interval));

    tracing::info!("scanprojects running at http://{}", addr);

    // Open dashboard in browser
    let url = format!("http://localhost:{}", port);
    if open::that(&url).is_err() {
        tracing::info!("Open {} in your browser", url);
    }

    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!(
                "Port {} is in use. Try: scanprojects --port <alt>\nError: {}",
                port, e
            );
            std::process::exit(1);
        }
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("Server error");
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

async fn handle_ws(mut socket: WebSocket, state: AppState) {
    // Send full sync on connect
    let projects = state.registry.get_all_projects();
    let sync = WsEvent::FullSync { data: projects };
    if let Ok(json) = serde_json::to_string(&sync) {
        let _ = socket.send(Message::Text(json.into())).await;
    }

    // Subscribe to events
    let mut rx = state.tx.subscribe();

    loop {
        tokio::select! {
            event = rx.recv() => {
                match event {
                    Ok(evt) => {
                        if let Ok(json) = serde_json::to_string(&evt) {
                            if socket.send(Message::Text(json.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {} // Ignore other messages from client
                }
            }
        }
    }
}

async fn serve_dashboard(uri: axum::http::Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    match DashboardAssets::get(path) {
        Some(file) => {
            let mime = mime_guess::from_path(path).first_or_text_plain();
            (
                [(axum::http::header::CONTENT_TYPE, mime.as_ref())],
                file.data.to_vec(),
            )
                .into_response()
        }
        None => {
            // SPA fallback — serve index.html for unmatched routes
            match DashboardAssets::get("index.html") {
                Some(file) => Html(String::from_utf8_lossy(&file.data).to_string()).into_response(),
                None => (axum::http::StatusCode::NOT_FOUND, "Dashboard not found").into_response(),
            }
        }
    }
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for ctrl-c");
    tracing::info!("Shutting down...");
}
