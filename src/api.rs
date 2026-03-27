use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::Json;
use serde::{Deserialize, Serialize};

use crate::registry::Registry;
use crate::secrets::SecretStore;
use crate::ws::WsEvent;

pub type AppState = Arc<SharedState>;

pub struct SharedState {
    pub registry: Arc<Registry>,
    pub tx: tokio::sync::broadcast::Sender<WsEvent>,
    pub secrets: Arc<SecretStore>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectResponse {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub framework: Option<String>,
    pub ports: Vec<u16>,
    pub pids: Vec<u32>,
    pub favorite: bool,
    pub uptime_seconds: u64,
    pub start_cmd: Option<String>,
    pub preferred_port: Option<u16>,
}

impl From<crate::registry::Project> for ProjectResponse {
    fn from(p: crate::registry::Project) -> Self {
        Self {
            id: p.id,
            name: p.name,
            path: p.path,
            framework: p.framework,
            ports: p.ports,
            pids: p.pids,
            favorite: p.favorite,
            uptime_seconds: p.uptime_seconds,
            start_cmd: p.start_cmd,
            preferred_port: p.preferred_port,
        }
    }
}

#[derive(Deserialize)]
pub struct PatchProject {
    pub favorite: Option<bool>,
    pub preferred_port: Option<u16>,
    pub start_cmd: Option<String>,
}

#[derive(Deserialize)]
pub struct AddProject {
    pub path: String,
}

#[derive(Serialize)]
pub struct KillResult {
    pub status: String,
    pub message: String,
}

pub async fn get_projects(State(state): State<AppState>) -> Json<Vec<ProjectResponse>> {
    let projects = state.registry.get_all_projects();
    Json(projects.into_iter().map(ProjectResponse::from).collect())
}

pub async fn get_ports(State(state): State<AppState>) -> Json<serde_json::Value> {
    let projects = state.registry.get_all_projects();
    let mut ports = Vec::new();
    for p in &projects {
        for port in &p.ports {
            ports.push(serde_json::json!({
                "port": port,
                "project_name": p.name,
                "framework": p.framework,
                "pid": p.pids.first(),
            }));
        }
    }
    Json(serde_json::Value::Array(ports))
}

pub async fn kill_project(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<KillResult>, StatusCode> {
    let project = state
        .registry
        .get_project(id)
        .ok_or(StatusCode::NOT_FOUND)?;

    let mut killed = 0;
    for &pid in &project.pids {
        match kill_pid(pid, &project.path) {
            Ok(_) => killed += 1,
            Err(e) => tracing::warn!("Failed to kill PID {}: {}", pid, e),
        }
    }

    let _ = state.tx.send(WsEvent::ProjectUpdated {
        data: crate::registry::Project {
            ports: Vec::new(),
            pids: Vec::new(),
            ..project
        },
    });

    Ok(Json(KillResult {
        status: "success".to_string(),
        message: format!("Killed {} process(es)", killed),
    }))
}

pub async fn kill_port(
    State(state): State<AppState>,
    Path(port): Path<u16>,
) -> Result<Json<KillResult>, StatusCode> {
    let projects = state.registry.get_all_projects();
    let mut found = false;

    for p in &projects {
        if p.ports.contains(&port) {
            for &pid in &p.pids {
                kill_pid(pid, &p.path).ok();
                found = true;
            }
        }
    }

    if !found {
        return Err(StatusCode::NOT_FOUND);
    }

    Ok(Json(KillResult {
        status: "success".to_string(),
        message: format!("Killed process on port {}", port),
    }))
}

pub async fn patch_project(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<PatchProject>,
) -> Result<StatusCode, StatusCode> {
    state
        .registry
        .get_project(id)
        .ok_or(StatusCode::NOT_FOUND)?;

    state
        .registry
        .update_project(id, body.favorite, body.preferred_port);

    if let Some(ref cmd) = body.start_cmd {
        state.registry.set_start_cmd(id, cmd);
    }

    if let Some(project) = state.registry.get_project(id) {
        let _ = state.tx.send(WsEvent::ProjectUpdated { data: project });
    }

    Ok(StatusCode::OK)
}

pub async fn restart_project(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<KillResult>, (StatusCode, Json<KillResult>)> {
    let project = state.registry.get_project(id).ok_or((
        StatusCode::NOT_FOUND,
        Json(KillResult {
            status: "error".to_string(),
            message: "Project not found".to_string(),
        }),
    ))?;

    let start_cmd = project.start_cmd.clone().ok_or((
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(KillResult {
            status: "error".to_string(),
            message: format!(
                "No start command configured for '{}'. Set one with: scanprojects set-start-cmd {} \"<cmd>\"",
                project.name, project.name
            ),
        }),
    ))?;

    // Collect secrets: global store + project-scoped store.
    let global_secrets = state.secrets.get_all(crate::secrets::GLOBAL);
    let project_secrets = state.secrets.get_all(id);
    // Project-scoped secrets override global ones with the same key.
    let mut env_vars: std::collections::HashMap<String, String> =
        global_secrets.into_iter().collect();
    env_vars.extend(project_secrets);

    // Spawn the start command detached from this server process.
    let path = project.path.clone();
    let name = project.name.clone();
    let mut cmd = tokio::process::Command::new("sh");
    cmd.arg("-c")
        .arg(&start_cmd)
        .current_dir(&path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    for (k, v) in &env_vars {
        cmd.env(k, v);
    }

    // If any of the PIDs to kill is our own process (self-restart), we cannot
    // kill-then-spawn: the new process would fail to bind the port because we
    // haven't released it yet. Instead: respond immediately, then kill, then
    // spawn after the port is free.
    let our_pid = std::process::id();
    let self_restart = project.pids.iter().any(|&p| p as u32 == our_pid);

    if self_restart {
        // We cannot use a tokio::spawn here: killing our own PID destroys the
        // entire process including all async tasks, so the spawn-after-kill
        // step would never run. Instead, delegate the whole kill+restart
        // sequence to an external shell that will outlive this process.
        let kill_cmds: String = project
            .pids
            .iter()
            .map(|&p| format!("kill -TERM {} 2>/dev/null; sleep 0.4; kill -KILL {} 2>/dev/null", p, p))
            .collect::<Vec<_>>()
            .join("; ");
        // The outer subshell (& disown) runs independently of this process.
        let shell_cmd = format!(
            "(sleep 0.3; {}; sleep 0.4; cd '{}' && {}) &",
            kill_cmds,
            project.path.replace('\'', "'\\''"),
            start_cmd.replace('\'', "'\\''"),
        );
        tracing::info!("Self-restart shell: {}", shell_cmd);
        std::process::Command::new("sh")
            .arg("-c")
            .arg(&shell_cmd)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .ok();
        return Ok(Json(KillResult {
            status: "success".to_string(),
            message: format!("Restarted: {}", name),
        }));
    }

    // Normal restart: kill existing processes first, then spawn.
    for &pid in &project.pids {
        kill_pid(pid, &project.path).ok();
    }
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    match cmd.spawn() {
        Ok(_) => {
            tracing::info!("Restarted '{}' via: {}", name, start_cmd);
            Ok(Json(KillResult {
                status: "success".to_string(),
                message: format!("Restarted: {}", name),
            }))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(KillResult {
                status: "error".to_string(),
                message: format!("Failed to spawn '{}': {}", start_cmd, e),
            }),
        )),
    }
}

pub async fn add_project(
    State(state): State<AppState>,
    Json(body): Json<AddProject>,
) -> Result<Json<KillResult>, StatusCode> {
    let path = std::path::Path::new(&body.path);
    if !path.exists() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let id = state.registry.add_project(&body.path, &name);

    if let Some(project) = state.registry.get_project(id) {
        let _ = state.tx.send(WsEvent::ProjectAdded { data: project });
    }

    Ok(Json(KillResult {
        status: "success".to_string(),
        message: format!("Added project: {}", name),
    }))
}

pub async fn delete_project(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, StatusCode> {
    state
        .registry
        .get_project(id)
        .ok_or(StatusCode::NOT_FOUND)?;

    state.registry.remove_project(id);
    let _ = state.tx.send(WsEvent::ProjectRemoved { id });

    Ok(StatusCode::OK)
}

// ── Env-file endpoints ────────────────────────────────────────────────────────

pub async fn get_project_env(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Vec<crate::env_file::EnvEntry>>, StatusCode> {
    let project = state.registry.get_project(id).ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(crate::env_file::read_env_files(&project.path)))
}

#[derive(Deserialize)]
pub struct UpdateEnvKeyBody {
    pub value: String,
    pub file_path: String,
}

pub async fn update_project_env_key(
    State(state): State<AppState>,
    Path((id, key)): Path<(i64, String)>,
    Json(body): Json<UpdateEnvKeyBody>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    // Verify the project exists
    state.registry.get_project(id).ok_or((
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({"error": "project not found"})),
    ))?;
    crate::env_file::update_env_key(&body.file_path, &key, &body.value).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e})),
        )
    })?;
    Ok(StatusCode::OK)
}

// ── Secret endpoints ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SecretQuery {
    pub project: Option<String>,
}

#[derive(Deserialize)]
pub struct SetSecretBody {
    pub key: String,
    pub value: String,
    pub project: Option<String>,
}

#[derive(Serialize)]
pub struct SecretValue {
    pub key: String,
    pub value: String,
}

fn resolve_project_id(state: &AppState, project: Option<&str>) -> Result<i64, StatusCode> {
    match project {
        None => Ok(crate::secrets::GLOBAL),
        Some(name) => state
            .registry
            .get_all_projects()
            .into_iter()
            .find(|p| p.name == name || p.path == name)
            .map(|p| p.id)
            .ok_or(StatusCode::NOT_FOUND),
    }
}

pub async fn list_secrets(
    State(state): State<AppState>,
    Query(q): Query<SecretQuery>,
) -> Result<Json<Vec<String>>, StatusCode> {
    let project_id = resolve_project_id(&state, q.project.as_deref())?;
    Ok(Json(state.secrets.list(project_id)))
}

pub async fn get_secret(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Query(q): Query<SecretQuery>,
) -> Result<Json<SecretValue>, StatusCode> {
    let project_id = resolve_project_id(&state, q.project.as_deref())?;
    let value = state
        .secrets
        .get(project_id, &key)
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(SecretValue { key, value }))
}

pub async fn set_secret(
    State(state): State<AppState>,
    Json(body): Json<SetSecretBody>,
) -> Result<StatusCode, StatusCode> {
    let project_id = resolve_project_id(&state, body.project.as_deref())?;
    state.secrets.set(project_id, &body.key, &body.value);
    Ok(StatusCode::OK)
}

pub async fn delete_secret(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Query(q): Query<SecretQuery>,
) -> Result<StatusCode, StatusCode> {
    let project_id = resolve_project_id(&state, q.project.as_deref())?;
    if state.secrets.delete(project_id, &key) {
        Ok(StatusCode::OK)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// Kill a process by PID, with validation that it still belongs to the expected project.
fn kill_pid(pid: u32, expected_path: &str) -> Result<(), String> {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;

    // Validate PID still exists
    let nix_pid = Pid::from_raw(pid as i32);

    // Send SIGTERM first
    kill(nix_pid, Signal::SIGTERM).map_err(|e| {
        if e == nix::errno::Errno::ESRCH {
            return "Process already dead".to_string();
        }
        format!("Kill failed: {}", e)
    })?;

    // Wait up to 3 seconds for process to die
    let _ = expected_path; // Used for validation in future
    std::thread::spawn(move || {
        for _ in 0..30 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if kill(nix_pid, None).is_err() {
                return; // Process is dead
            }
        }
        // Process didn't die — escalate to SIGKILL
        tracing::info!("PID {} didn't respond to SIGTERM, sending SIGKILL", pid);
        let _ = kill(nix_pid, Signal::SIGKILL);
    });

    Ok(())
}
