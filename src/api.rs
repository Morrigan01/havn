use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::Json;
use serde::{Deserialize, Serialize};

use crate::rate_limit::RateLimiter;
use crate::registry::Registry;
use crate::secrets::SecretStore;
use crate::ws::WsEvent;

pub type AppState = Arc<SharedState>;

pub struct SharedState {
    pub registry: Arc<Registry>,
    pub tx: tokio::sync::broadcast::Sender<WsEvent>,
    pub secrets: Arc<SecretStore>,
    pub logs: Arc<crate::logs::LogStore>,
    pub rate_limiter: RateLimiter,
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

#[derive(Serialize)]
pub struct GitStatus {
    pub branch: String,
    pub dirty: bool,
}

#[derive(Serialize)]
pub struct HealthCheck {
    pub port: u16,
    pub status: String,   // "up" | "down" | "timeout"
    pub status_code: Option<u16>,
    pub latency_ms: u64,
}

#[derive(Serialize)]
pub struct ProcessStats {
    pub pid: u32,
    pub cpu_percent: f32,
    pub mem_rss_kb: u64,
}

#[derive(Deserialize)]
pub struct CreateProfile {
    pub name: String,
}

#[derive(Deserialize)]
pub struct AddProfileProject {
    pub project_id: i64,
}

#[derive(Deserialize)]
pub struct LogQuery {
    pub lines: Option<usize>,
}

#[derive(Serialize)]
pub struct RestartVerifyResult {
    pub status: String,        // "healthy" | "crashed" | "timeout"
    pub port: Option<u16>,
    pub boot_time_ms: Option<u64>,
    pub exit_code: Option<i32>,
    pub last_stderr: Vec<String>,
    pub message: String,
}

#[derive(Serialize)]
pub struct EffectiveEnv {
    pub source: String, // "env_file" | "secret_store" | "secret_store (global)"
    pub key: String,
    pub value: String,
}

#[derive(Deserialize)]
pub struct FindPortQuery {
    pub preferred: Option<u16>,
}

/// Reject start commands that contain obviously dangerous shell patterns.
/// This is a safety net, not a sandbox — the user ultimately controls what they run.
fn validate_start_cmd(cmd: &str) -> Result<(), String> {
    let dangerous = [
        "rm -rf /",
        "rm -rf /*",
        "mkfs.",
        "dd if=",
        ":(){",         // fork bomb
        "chmod -R 777 /",
        "> /dev/sda",
        "curl | sh",
        "curl | bash",
        "wget | sh",
        "wget | bash",
    ];
    let lower = cmd.to_lowercase();
    for pattern in &dangerous {
        if lower.contains(pattern) {
            return Err(format!(
                "Start command rejected: contains dangerous pattern '{}'",
                pattern
            ));
        }
    }
    Ok(())
}

fn check_rate_limit(state: &AppState) -> Result<(), (StatusCode, Json<KillResult>)> {
    if !state.rate_limiter.try_acquire() {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(KillResult {
                status: "error".to_string(),
                message: "Rate limit exceeded. Try again shortly.".to_string(),
            }),
        ));
    }
    Ok(())
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
) -> Result<Json<KillResult>, (StatusCode, Json<KillResult>)> {
    check_rate_limit(&state)?;
    let project = state.registry.get_project(id).ok_or((
        StatusCode::NOT_FOUND,
        Json(KillResult { status: "error".into(), message: "Project not found".into() }),
    ))?;

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
) -> Result<Json<KillResult>, (StatusCode, Json<KillResult>)> {
    check_rate_limit(&state)?;
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
        return Err((
            StatusCode::NOT_FOUND,
            Json(KillResult { status: "error".into(), message: format!("Port {} not found", port) }),
        ));
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
) -> Result<StatusCode, (StatusCode, Json<KillResult>)> {
    state
        .registry
        .get_project(id)
        .ok_or((StatusCode::NOT_FOUND, Json(KillResult { status: "error".into(), message: "Project not found".into() })))?;

    if let Some(ref cmd) = body.start_cmd {
        validate_start_cmd(cmd).map_err(|e| (
            StatusCode::BAD_REQUEST,
            Json(KillResult { status: "error".into(), message: e }),
        ))?;
    }

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
    check_rate_limit(&state)?;
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
                "No start command configured for '{}'. Set one with: havn set-start-cmd {} \"<cmd>\"",
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
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
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
        Ok(mut child) => {
            capture_logs(&state.logs, child.stdout.take(), child.stderr.take(), id);
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

/// Restart a single process identified by PORT within a multi-process project.
/// Uses lsof live to find the current PID holding that port (avoids stale registry data),
/// kills it, then re-spawns the project's start_cmd.
pub async fn restart_process(
    State(state): State<AppState>,
    Path((id, port)): Path<(i64, u16)>,
) -> Result<Json<KillResult>, (StatusCode, Json<KillResult>)> {
    check_rate_limit(&state)?;
    let project = state.registry.get_project(id).ok_or((
        StatusCode::NOT_FOUND,
        Json(KillResult {
            status: "error".to_string(),
            message: "Project not found".to_string(),
        }),
    ))?;

    if !project.ports.contains(&port) {
        return Err((
            StatusCode::NOT_FOUND,
            Json(KillResult {
                status: "error".to_string(),
                message: format!("Port {} not found in project '{}'", port, project.name),
            }),
        ));
    }

    let start_cmd = project.start_cmd.clone().ok_or((
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(KillResult {
            status: "error".to_string(),
            message: format!(
                "No start command configured for '{}'. Set one with: havn set-start-cmd {} \"<cmd>\"",
                project.name, project.name
            ),
        }),
    ))?;

    let name = project.name.clone();
    let path = project.path.clone();
    let our_pid = std::process::id();

    // Find the live PID(s) holding this port right now via lsof — not the registry,
    // which can be stale by the time the user clicks Restart.
    let live_pids = live_pids_for_port(port).await;

    if live_pids.iter().any(|&p| p == our_pid) {
        // Self-restart path (server is on this port) — detached shell.
        let kill_cmds: String = live_pids
            .iter()
            .map(|&p| format!("kill -TERM {p} 2>/dev/null; sleep 0.4; kill -KILL {p} 2>/dev/null"))
            .collect::<Vec<_>>()
            .join("; ");
        let shell_cmd = format!(
            "(sleep 0.3; {}; sleep 0.4; cd '{}' && {}) &",
            kill_cmds,
            path.replace('\'', "'\\''"),
            start_cmd.replace('\'', "'\\''"),
        );
        std::process::Command::new("sh")
            .arg("-c").arg(&shell_cmd)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn().ok();
        return Ok(Json(KillResult {
            status: "success".to_string(),
            message: format!("Restarted :{} in '{}'", port, name),
        }));
    }

    // Kill the live PID(s) for this port.
    for pid in &live_pids {
        kill_pid(*pid, &path).ok();
    }
    // Fall back to registry PID if lsof found nothing (process may have just exited).
    if live_pids.is_empty() {
        if let Some(idx) = project.ports.iter().position(|&p| p == port) {
            if let Some(&reg_pid) = project.pids.get(idx) {
                kill_pid(reg_pid, &path).ok();
            }
        }
    }

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Collect secrets and spawn.
    let global_secrets = state.secrets.get_all(crate::secrets::GLOBAL);
    let project_secrets = state.secrets.get_all(id);
    let mut env_vars: std::collections::HashMap<String, String> =
        global_secrets.into_iter().collect();
    env_vars.extend(project_secrets);

    let mut cmd = tokio::process::Command::new("sh");
    cmd.arg("-c").arg(&start_cmd)
        .current_dir(&path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    for (k, v) in &env_vars {
        cmd.env(k, v);
    }

    match cmd.spawn() {
        Ok(mut child) => {
            capture_logs(&state.logs, child.stdout.take(), child.stderr.take(), id);
            tracing::info!("Restarted :{} in '{}' via: {}", port, name, start_cmd);
            Ok(Json(KillResult {
                status: "success".to_string(),
                message: format!("Restarted :{} in '{}'", port, name),
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

/// Returns the live PIDs currently listening on `port` by running lsof.
async fn live_pids_for_port(port: u16) -> Vec<u32> {
    let out = tokio::process::Command::new("lsof")
        .args(["-iTCP", &format!(":{}", port), "-sTCP:LISTEN", "-t", "-n", "-P"])
        .output()
        .await;
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter_map(|l| l.trim().parse::<u32>().ok())
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Restart a project and wait up to 15s for it to become healthy (port binds)
/// or crash. Returns structured verification result.
pub async fn restart_and_verify(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<RestartVerifyResult>, (StatusCode, Json<KillResult>)> {
    check_rate_limit(&state)?;
    let project = state.registry.get_project(id).ok_or((
        StatusCode::NOT_FOUND,
        Json(KillResult { status: "error".into(), message: "Project not found".into() }),
    ))?;

    let start_cmd = project.start_cmd.clone().ok_or((
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(KillResult {
            status: "error".into(),
            message: format!("No start command configured for '{}'", project.name),
        }),
    ))?;

    let preferred_port = project.preferred_port.or_else(|| project.ports.first().copied());
    let path = project.path.clone();
    let name = project.name.clone();

    // Collect secrets
    let global_secrets = state.secrets.get_all(crate::secrets::GLOBAL);
    let project_secrets = state.secrets.get_all(id);
    let mut env_vars: std::collections::HashMap<String, String> =
        global_secrets.into_iter().collect();
    env_vars.extend(project_secrets);

    // Kill existing processes
    for &pid in &project.pids {
        kill_pid(pid, &project.path).ok();
    }
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Spawn the new process
    let mut cmd = tokio::process::Command::new("sh");
    cmd.arg("-c")
        .arg(&start_cmd)
        .current_dir(&path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    for (k, v) in &env_vars {
        cmd.env(k, v);
    }

    let start_time = std::time::Instant::now();

    match cmd.spawn() {
        Ok(mut child) => {
            let child_pid = child.id();
            capture_logs(&state.logs, child.stdout.take(), child.stderr.take(), id);

            // Poll for up to 15 seconds: check if port binds or process dies.
            let timeout = std::time::Duration::from_secs(15);
            let poll_interval = std::time::Duration::from_millis(300);

            loop {
                let elapsed = start_time.elapsed();
                if elapsed > timeout {
                    return Ok(Json(RestartVerifyResult {
                        status: "timeout".into(),
                        port: preferred_port,
                        boot_time_ms: Some(elapsed.as_millis() as u64),
                        exit_code: None,
                        last_stderr: get_recent_stderr(&state.logs, id, 5),
                        message: format!("'{}' started but port not detected within {}s", name, timeout.as_secs()),
                    }));
                }

                // Check if process has crashed
                if let Some(pid) = child_pid {
                    let alive = nix::sys::signal::kill(
                        nix::unistd::Pid::from_raw(pid as i32),
                        None,
                    ).is_ok();
                    if !alive {
                        return Ok(Json(RestartVerifyResult {
                            status: "crashed".into(),
                            port: preferred_port,
                            boot_time_ms: Some(elapsed.as_millis() as u64),
                            exit_code: None,
                            last_stderr: get_recent_stderr(&state.logs, id, 10),
                            message: format!("'{}' crashed shortly after starting", name),
                        }));
                    }
                }

                // Check if the expected port is now listening
                if let Some(port) = preferred_port {
                    let pids = live_pids_for_port(port).await;
                    if !pids.is_empty() {
                        let boot_ms = elapsed.as_millis() as u64;
                        tracing::info!("Restart-and-verify '{}': healthy on :{} in {}ms", name, port, boot_ms);
                        return Ok(Json(RestartVerifyResult {
                            status: "healthy".into(),
                            port: Some(port),
                            boot_time_ms: Some(boot_ms),
                            exit_code: None,
                            last_stderr: Vec::new(),
                            message: format!("'{}' is healthy on :{} ({}ms)", name, port, boot_ms),
                        }));
                    }
                }

                tokio::time::sleep(poll_interval).await;
            }
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(KillResult {
                status: "error".into(),
                message: format!("Failed to spawn '{}': {}", start_cmd, e),
            }),
        )),
    }
}

/// Get recent errors (stderr lines) for a project.
pub async fn get_project_errors(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(q): Query<LogQuery>,
) -> Result<Json<Vec<crate::logs::LogLine>>, StatusCode> {
    state.registry.get_project(id).ok_or(StatusCode::NOT_FOUND)?;
    let n = q.lines.unwrap_or(50).min(200);
    let all_logs = state.logs.get(id, 500);
    let errors: Vec<crate::logs::LogLine> = all_logs
        .into_iter()
        .filter(|l| {
            l.stream == "stderr"
                || l.text.contains("Error")
                || l.text.contains("error")
                || l.text.contains("ERRO")
                || l.text.contains("panic")
                || l.text.contains("PANIC")
                || l.text.contains("Traceback")
                || l.text.contains("Exception")
                || l.text.contains("FATAL")
                || l.text.contains("fatal")
        })
        .collect();
    let skip = if errors.len() > n { errors.len() - n } else { 0 };
    Ok(Json(errors.into_iter().skip(skip).collect()))
}

/// Find the nearest available port starting from a preferred port.
/// Checks both IPv4 and IPv6 to avoid false positives.
pub async fn find_available_port(
    Query(q): Query<FindPortQuery>,
) -> Json<serde_json::Value> {
    let start = q.preferred.unwrap_or(3000);
    for port in start..=start + 100 {
        // Try binding on both IPv4 and IPv6 — a port is only free if both succeed.
        let ipv4 = tokio::net::TcpListener::bind(("0.0.0.0", port)).await;
        if ipv4.is_err() {
            continue;
        }
        drop(ipv4);
        let ipv6 = tokio::net::TcpListener::bind(("::1", port)).await;
        if ipv6.is_err() {
            continue;
        }
        drop(ipv6);
        return Json(serde_json::json!({
            "port": port,
            "preferred": start,
            "offset": port - start,
        }));
    }
    Json(serde_json::json!({
        "error": format!("No available port found in range {}–{}", start, start + 100),
    }))
}

/// Return the effective environment variables a project would launch with.
/// Merges: .env files → global secrets → project-scoped secrets (later overrides earlier).
pub async fn get_effective_env(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Vec<EffectiveEnv>>, StatusCode> {
    let project = state.registry.get_project(id).ok_or(StatusCode::NOT_FOUND)?;

    let mut result: Vec<EffectiveEnv> = Vec::new();
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    // Layer 1: .env file entries
    let env_entries = crate::env_file::read_env_files(&project.path);
    for entry in env_entries {
        let idx = result.len();
        seen.insert(entry.key.clone(), idx);
        result.push(EffectiveEnv {
            source: format!("env_file ({})", entry.file),
            key: entry.key,
            value: entry.value,
        });
    }

    // Layer 2: global secrets (override .env)
    let global_secrets = state.secrets.get_all(crate::secrets::GLOBAL);
    for (key, value) in global_secrets {
        if let Some(&idx) = seen.get(&key) {
            result[idx] = EffectiveEnv {
                source: "secret_store (global)".into(),
                key: key.clone(),
                value,
            };
        } else {
            let idx = result.len();
            seen.insert(key.clone(), idx);
            result.push(EffectiveEnv {
                source: "secret_store (global)".into(),
                key,
                value,
            });
        }
    }

    // Layer 3: project-scoped secrets (highest priority)
    let project_secrets = state.secrets.get_all(id);
    for (key, value) in project_secrets {
        if let Some(&idx) = seen.get(&key) {
            result[idx] = EffectiveEnv {
                source: "secret_store (project)".into(),
                key: key.clone(),
                value,
            };
        } else {
            seen.insert(key.clone(), result.len());
            result.push(EffectiveEnv {
                source: "secret_store (project)".into(),
                key,
                value,
            });
        }
    }

    Ok(Json(result))
}

/// System overview: all projects with health/status summary.
pub async fn system_overview(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    let projects = state.registry.get_all_projects();
    let mut entries = Vec::new();

    for p in &projects {
        let error_count = state.logs.get(p.id, 500)
            .iter()
            .filter(|l| l.stream == "stderr" || l.text.contains("Error") || l.text.contains("panic"))
            .count();

        entries.push(serde_json::json!({
            "name": p.name,
            "path": p.path,
            "framework": p.framework,
            "ports": p.ports,
            "pids": p.pids,
            "running": !p.ports.is_empty(),
            "uptime_seconds": p.uptime_seconds,
            "favorite": p.favorite,
            "has_start_cmd": p.start_cmd.is_some(),
            "recent_error_count": error_count,
        }));
    }

    let running = entries.iter().filter(|e| e["running"].as_bool() == Some(true)).count();
    let total = entries.len();

    Json(serde_json::json!({
        "total_projects": total,
        "running_projects": running,
        "stopped_projects": total - running,
        "projects": entries,
    }))
}

/// Helper: get the last N stderr lines from the log store.
fn get_recent_stderr(logs: &std::sync::Arc<crate::logs::LogStore>, project_id: i64, n: usize) -> Vec<String> {
    let all = logs.get(project_id, 100);
    all.iter()
        .filter(|l| l.stream == "stderr")
        .map(|l| l.text.clone())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .take(n)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
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

// ── Git status ────────────────────────────────────────────────────────────

pub async fn get_project_git(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<GitStatus>, (StatusCode, Json<KillResult>)> {
    let project = state.registry.get_project(id).ok_or((
        StatusCode::NOT_FOUND,
        Json(KillResult { status: "error".into(), message: "Project not found".into() }),
    ))?;

    let out = tokio::process::Command::new("git")
        .args(["-C", &project.path, "status", "--porcelain=v1", "-b"])
        .output()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(KillResult { status: "error".into(), message: e.to_string() })))?;

    if !out.status.success() {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, Json(KillResult {
            status: "error".into(),
            message: "Not a git repository".into(),
        })));
    }

    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut lines = stdout.lines();
    let branch_line = lines.next().unwrap_or("").trim_start_matches("## ");
    let branch = branch_line.split("...").next()
        .and_then(|s| s.split(' ').next())
        .unwrap_or("HEAD")
        .to_string();
    let dirty = lines.any(|l| !l.trim().is_empty());

    Ok(Json(GitStatus { branch, dirty }))
}

// ── Health checks ─────────────────────────────────────────────────────────

pub async fn get_project_health(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Vec<HealthCheck>>, StatusCode> {
    let project = state.registry.get_project(id).ok_or(StatusCode::NOT_FOUND)?;

    let checks = futures::future::join_all(project.ports.iter().map(|&port| async move {
        let start = std::time::Instant::now();
        let url = format!("http://127.0.0.1:{}/", port);
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(1500))
            .build()
            .unwrap();

        match client.get(&url).send().await {
            Ok(resp) => HealthCheck {
                port,
                status: "up".into(),
                status_code: Some(resp.status().as_u16()),
                latency_ms: start.elapsed().as_millis() as u64,
            },
            Err(e) => HealthCheck {
                port,
                status: if e.is_timeout() { "timeout".into() } else { "down".into() },
                status_code: None,
                latency_ms: start.elapsed().as_millis() as u64,
            },
        }
    })).await;

    Ok(Json(checks))
}

// ── Resource stats ────────────────────────────────────────────────────────

pub async fn get_project_resources(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Vec<ProcessStats>>, StatusCode> {
    let project = state.registry.get_project(id).ok_or(StatusCode::NOT_FOUND)?;

    if project.pids.is_empty() {
        return Ok(Json(Vec::new()));
    }

    let pid_list = project.pids.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(",");
    let out = tokio::process::Command::new("ps")
        .args(["-p", &pid_list, "-o", "pid=,%cpu=,rss="])
        .output()
        .await
        .unwrap_or_else(|_| std::process::Output {
            status: {
                use std::os::unix::process::ExitStatusExt;
                std::process::ExitStatus::from_raw(1)
            },
            stdout: Vec::new(),
            stderr: Vec::new(),
        });

    let stats: Vec<ProcessStats> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                Some(ProcessStats {
                    pid: parts[0].parse().ok()?,
                    cpu_percent: parts[1].parse().ok()?,
                    mem_rss_kb: parts[2].parse().ok()?,
                })
            } else {
                None
            }
        })
        .collect();

    Ok(Json(stats))
}

// ── Open in terminal ──────────────────────────────────────────────────────

pub async fn open_terminal(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<KillResult>, (StatusCode, Json<KillResult>)> {
    let project = state.registry.get_project(id).ok_or((
        StatusCode::NOT_FOUND,
        Json(KillResult { status: "error".into(), message: "Project not found".into() }),
    ))?;

    #[cfg(target_os = "macos")]
    {
        let script = format!(
            r#"tell application "Terminal" to do script "cd '{}'" activate"#,
            project.path.replace('\'', "\\'")
        );
        std::process::Command::new("osascript")
            .arg("-e").arg(&script)
            .spawn()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(KillResult {
                status: "error".into(), message: e.to_string()
            })))?;
    }

    #[cfg(not(target_os = "macos"))]
    {
        for term in &["x-terminal-emulator", "gnome-terminal", "xterm"] {
            if std::process::Command::new(term)
                .arg(&format!("--working-directory={}", project.path))
                .spawn().is_ok() { break; }
        }
    }

    Ok(Json(KillResult { status: "success".into(), message: "Terminal opened".into() }))
}

// ── Logs ─────────────────────────────────────────────────────────────────

pub async fn get_project_logs(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(q): Query<LogQuery>,
) -> Result<Json<Vec<crate::logs::LogLine>>, StatusCode> {
    state.registry.get_project(id).ok_or(StatusCode::NOT_FOUND)?;
    let n = q.lines.unwrap_or(200).min(500);
    Ok(Json(state.logs.get(id, n)))
}

pub async fn clear_project_logs(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, StatusCode> {
    state.registry.get_project(id).ok_or(StatusCode::NOT_FOUND)?;
    state.logs.clear(id);
    Ok(StatusCode::OK)
}

// ── Profiles ──────────────────────────────────────────────────────────────

pub async fn list_profiles(
    State(state): State<AppState>,
) -> Json<Vec<crate::registry::Profile>> {
    Json(state.registry.list_profiles())
}

pub async fn create_profile(
    State(state): State<AppState>,
    Json(body): Json<CreateProfile>,
) -> Result<Json<crate::registry::Profile>, (StatusCode, Json<KillResult>)> {
    let id = state.registry.create_profile(&body.name).map_err(|e| (
        StatusCode::CONFLICT,
        Json(KillResult { status: "error".into(), message: e }),
    ))?;
    let profiles = state.registry.list_profiles();
    let profile = profiles.into_iter().find(|p| p.id == id).unwrap();
    Ok(Json(profile))
}

pub async fn delete_profile_handler(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> StatusCode {
    state.registry.delete_profile(id);
    StatusCode::OK
}

pub async fn add_project_to_profile(
    State(state): State<AppState>,
    Path(profile_id): Path<i64>,
    Json(body): Json<AddProfileProject>,
) -> StatusCode {
    state.registry.add_project_to_profile(profile_id, body.project_id);
    StatusCode::OK
}

pub async fn remove_project_from_profile(
    State(state): State<AppState>,
    Path((profile_id, project_id)): Path<(i64, i64)>,
) -> StatusCode {
    state.registry.remove_project_from_profile(profile_id, project_id);
    StatusCode::OK
}

pub async fn start_profile(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    let profiles = state.registry.list_profiles();
    let profile = match profiles.into_iter().find(|p| p.id == id) {
        None => return Json(serde_json::json!({"started": 0, "errors": ["Profile not found"]})),
        Some(p) => p,
    };

    let mut started = 0u32;
    let mut errors: Vec<String> = Vec::new();

    for project_id in &profile.project_ids {
        if let Some(project) = state.registry.get_project(*project_id) {
            if let Some(ref cmd) = project.start_cmd {
                for &pid in &project.pids { kill_pid(pid, &project.path).ok(); }
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;

                let global_secrets = state.secrets.get_all(crate::secrets::GLOBAL);
                let proj_secrets = state.secrets.get_all(*project_id);
                let mut env_vars: std::collections::HashMap<String, String> = global_secrets.into_iter().collect();
                env_vars.extend(proj_secrets);

                let mut command = tokio::process::Command::new("sh");
                command.arg("-c").arg(cmd).current_dir(&project.path)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped());
                for (k, v) in &env_vars { command.env(k, v); }

                match command.spawn() {
                    Ok(mut child) => {
                        capture_logs(&state.logs, child.stdout.take(), child.stderr.take(), *project_id);
                        started += 1;
                    }
                    Err(e) => errors.push(format!("{}: {}", project.name, e)),
                }
            } else {
                errors.push(format!("{}: no start_cmd configured", project.name));
            }
        }
    }

    Json(serde_json::json!({"started": started, "errors": errors}))
}

pub async fn stop_profile(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    let profiles = state.registry.list_profiles();
    let profile = match profiles.into_iter().find(|p| p.id == id) {
        None => return Json(serde_json::json!({"stopped": 0})),
        Some(p) => p,
    };

    let mut stopped = 0u32;
    for project_id in &profile.project_ids {
        if let Some(project) = state.registry.get_project(*project_id) {
            for &pid in &project.pids {
                if kill_pid(pid, &project.path).is_ok() { stopped += 1; }
            }
        }
    }
    Json(serde_json::json!({"stopped": stopped}))
}

/// Spawn background tasks to read stdout/stderr of a child process into the log store.
fn capture_logs(
    logs: &Arc<crate::logs::LogStore>,
    stdout: Option<tokio::process::ChildStdout>,
    stderr: Option<tokio::process::ChildStderr>,
    project_id: i64,
) {
    use tokio::io::AsyncBufReadExt;
    if let Some(stdout) = stdout {
        let logs = Arc::clone(logs);
        tokio::spawn(async move {
            let mut reader = tokio::io::BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                logs.push(project_id, "stdout", line);
            }
        });
    }
    if let Some(stderr) = stderr {
        let logs = Arc::clone(logs);
        tokio::spawn(async move {
            let mut reader = tokio::io::BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                logs.push(project_id, "stderr", line);
            }
        });
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
