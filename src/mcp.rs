use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    schemars::JsonSchema,
    tool, tool_handler, tool_router,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct McpServer {
    api_url: String,
    tool_router: ToolRouter<Self>,
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for McpServer {}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ListProjectsParams {}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct KillPortParams {
    /// The port number to kill
    pub port: u16,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct GetProjectParams {
    /// Project name
    pub name: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct FindPortParams {
    /// Project name to look up
    pub project: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct RestartProjectParams {
    /// Project name to restart
    pub name: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ListSecretsParams {
    /// Project name to scope to (omit for global secrets)
    pub project: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct GetSecretParams {
    /// Secret key
    pub key: String,
    /// Project name to scope to (omit for global secrets)
    pub project: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct SetSecretParams {
    /// Secret key
    pub key: String,
    /// Secret value (will be encrypted at rest)
    pub value: String,
    /// Project name to scope to (omit for global secrets)
    pub project: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct RestartAndVerifyParams {
    /// Project name to restart and verify
    pub name: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct GetErrorsParams {
    /// Project name
    pub name: String,
    /// Maximum number of error lines to return (default: 20)
    pub lines: Option<usize>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct FindAvailablePortParams {
    /// Preferred starting port (default: 3000)
    pub preferred: Option<u16>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct SystemOverviewParams {}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct GetEffectiveEnvParams {
    /// Project name
    pub name: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct GetVersionParams {}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct GetLogsParams {
    /// Project name
    pub name: String,
    /// Number of log lines to return (default: 50, max: 500)
    pub lines: Option<usize>,
    /// Filter by stream: "stdout", "stderr", or "all" (default: "all")
    pub stream: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct RunCommandParams {
    /// Project name (command runs in the project's directory)
    pub name: String,
    /// Shell command to run (e.g. "npm install", "cargo build", "npx prisma migrate dev")
    pub command: String,
    /// Timeout in seconds (default: 30, max: 300)
    pub timeout_secs: Option<u64>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct HealthCheckParams {
    /// Port number to check
    pub port: u16,
    /// HTTP path to check (default: "/")
    pub path: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DockerStatusParams {}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct GetResourcesParams {
    /// Project name
    pub name: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct CheckDepsParams {
    /// Project name
    pub name: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DbStatusParams {
    /// Project name
    pub name: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ListStacksParams {}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct GetStackParams {
    /// Stack (profile) name
    pub name: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct StartStackParams {
    /// Stack (profile) name to start
    pub name: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct StopStackParams {
    /// Stack (profile) name to stop
    pub name: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DiagnoseStackParams {
    /// Stack (profile) name to diagnose
    pub name: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ValidateEnvParams {
    /// Stack (profile) name to validate
    pub name: String,
}

#[tool_router]
impl McpServer {
    pub fn new(api_url: String) -> Self {
        Self {
            api_url,
            tool_router: Self::tool_router(),
        }
    }

    /// List all detected projects with their ports, frameworks, and status.
    #[tool(name = "list_projects", description = "List all running projects and their ports")]
    async fn list_projects(&self, _params: Parameters<ListProjectsParams>) -> String {
        match reqwest::get(&format!("{}/projects", self.api_url)).await {
            Ok(resp) => match resp.text().await {
                Ok(body) => body,
                Err(e) => format!("Error reading response: {}", e),
            },
            Err(_) => "havn server not running. Start with `havn` or `havn install-service`.".to_string(),
        }
    }

    /// Get details about a specific project by name.
    #[tool(name = "get_project", description = "Get details about a specific project")]
    async fn get_project(&self, params: Parameters<GetProjectParams>) -> String {
        let projects_resp = match reqwest::get(&format!("{}/projects", self.api_url)).await {
            Ok(r) => r,
            Err(_) => return "havn server not running.".to_string(),
        };

        let projects: Vec<serde_json::Value> = match projects_resp.json().await {
            Ok(p) => p,
            Err(e) => return format!("Error: {}", e),
        };

        match projects.iter().find(|p| {
            p.get("name").and_then(|n| n.as_str()) == Some(&params.0.name)
        }) {
            Some(project) => serde_json::to_string_pretty(project).unwrap_or_default(),
            None => format!("Project '{}' not found.", params.0.name),
        }
    }

    /// Kill a process running on a specific port.
    #[tool(name = "kill_port", description = "Kill the process running on a specific port")]
    async fn kill_port(&self, params: Parameters<KillPortParams>) -> String {
        let client = reqwest::Client::new();
        match client
            .post(format!("{}/kill/{}", self.api_url, params.0.port))
            .send()
            .await
        {
            Ok(resp) => match resp.text().await {
                Ok(body) => body,
                Err(e) => format!("Error: {}", e),
            },
            Err(_) => "havn server not running.".to_string(),
        }
    }

    /// Kill a running project and restart it using its configured start command.
    #[tool(
        name = "restart_project",
        description = "Kill and restart a project using its configured start command"
    )]
    async fn restart_project(&self, params: Parameters<RestartProjectParams>) -> String {
        let projects_resp = match reqwest::get(&format!("{}/projects", self.api_url)).await {
            Ok(r) => r,
            Err(_) => return "havn server not running.".to_string(),
        };
        let projects: Vec<serde_json::Value> = match projects_resp.json().await {
            Ok(p) => p,
            Err(e) => return format!("Error: {}", e),
        };
        match projects
            .iter()
            .find(|p| p.get("name").and_then(|n| n.as_str()) == Some(&params.0.name))
        {
            Some(project) => {
                let id = project.get("id").and_then(|i| i.as_i64()).unwrap_or(0);
                let client = reqwest::Client::new();
                match client
                    .post(format!("{}/projects/{}/restart", self.api_url, id))
                    .send()
                    .await
                {
                    Ok(resp) => resp.text().await.unwrap_or_default(),
                    Err(e) => format!("Error: {}", e),
                }
            }
            None => format!("Project '{}' not found.", params.0.name),
        }
    }

    /// List stored secret keys for a project (values are not returned).
    #[tool(
        name = "list_secrets",
        description = "List secret keys stored for a project (values are never returned)"
    )]
    async fn list_secrets(&self, params: Parameters<ListSecretsParams>) -> String {
        let mut url = format!("{}/secrets", self.api_url);
        if let Some(ref p) = params.0.project {
            url = format!("{}?project={}", url, p);
        }
        match reqwest::get(&url).await {
            Ok(resp) => resp.text().await.unwrap_or_default(),
            Err(_) => "havn server not running.".to_string(),
        }
    }

    /// Get a decrypted secret value by key.
    #[tool(
        name = "get_secret",
        description = "Retrieve a decrypted secret value by key"
    )]
    async fn get_secret(&self, params: Parameters<GetSecretParams>) -> String {
        let mut url = format!("{}/secrets/{}", self.api_url, params.0.key);
        if let Some(ref p) = params.0.project {
            url = format!("{}?project={}", url, p);
        }
        let client = reqwest::Client::new();
        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                let data: serde_json::Value = resp.json().await.unwrap_or_default();
                data.get("value")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            }
            Ok(resp) if resp.status() == 404 => {
                format!("Secret '{}' not found.", params.0.key)
            }
            Ok(_) => "Error retrieving secret.".to_string(),
            Err(_) => "havn server not running.".to_string(),
        }
    }

    /// Store an encrypted secret.
    #[tool(
        name = "set_secret",
        description = "Store an encrypted secret (AES-256-GCM, persisted locally)"
    )]
    async fn set_secret(&self, params: Parameters<SetSecretParams>) -> String {
        let mut body = serde_json::json!({
            "key": params.0.key,
            "value": params.0.value,
        });
        if let Some(ref p) = params.0.project {
            body["project"] = serde_json::json!(p);
        }
        let client = reqwest::Client::new();
        match client
            .post(format!("{}/secrets", self.api_url))
            .json(&body)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                format!("Secret '{}' stored.", params.0.key)
            }
            Ok(resp) => resp.text().await.unwrap_or_default(),
            Err(_) => "havn server not running.".to_string(),
        }
    }

    /// Find which port a named project is running on.
    #[tool(
        name = "find_port_for_project",
        description = "Find which port(s) a project is running on"
    )]
    async fn find_port_for_project(&self, params: Parameters<FindPortParams>) -> String {
        let projects_resp = match reqwest::get(&format!("{}/projects", self.api_url)).await {
            Ok(r) => r,
            Err(_) => return "havn server not running.".to_string(),
        };

        let projects: Vec<serde_json::Value> = match projects_resp.json().await {
            Ok(p) => p,
            Err(e) => return format!("Error: {}", e),
        };

        match projects.iter().find(|p| {
            p.get("name").and_then(|n| n.as_str()) == Some(&params.0.project)
        }) {
            Some(project) => {
                let ports = project
                    .get("ports")
                    .and_then(|p| p.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_u64())
                            .map(|p| format!(":{}", p))
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_else(|| "no ports".to_string());
                format!("{} is running on {}", params.0.project, ports)
            }
            None => format!("Project '{}' not found.", params.0.project),
        }
    }

    /// Restart a project and wait for it to become healthy (port binds) or detect a crash.
    /// Returns structured verification: status (healthy/crashed/timeout), boot time, port, and recent stderr on failure.
    /// This is the preferred restart method for AI agents — it gives a definitive answer instead of fire-and-forget.
    #[tool(
        name = "restart_and_verify",
        description = "Restart a project and verify it becomes healthy. Returns status (healthy/crashed/timeout), boot time, and stderr on failure. Use this instead of restart_project when you need to confirm the server is actually running."
    )]
    async fn restart_and_verify(&self, params: Parameters<RestartAndVerifyParams>) -> String {
        let id = match self.resolve_project_id(&params.0.name).await {
            Ok(id) => id,
            Err(e) => return e,
        };
        let client = reqwest::Client::new();
        match client
            .post(format!("{}/projects/{}/restart-and-verify", self.api_url, id))
            .timeout(std::time::Duration::from_secs(20))
            .send()
            .await
        {
            Ok(resp) => match resp.text().await {
                Ok(body) => body,
                Err(e) => format!("Error reading response: {}", e),
            },
            Err(e) if e.is_timeout() => {
                format!("Verification timed out — server may still be starting. Check with get_project.")
            }
            Err(_) => "havn server not running.".to_string(),
        }
    }

    /// Get recent errors and stderr output for a project.
    /// Useful after making code changes to check if the server is throwing errors.
    #[tool(
        name = "get_errors",
        description = "Get recent errors (stderr lines, exceptions, panics) for a project. Use this after code changes to check for problems."
    )]
    async fn get_errors(&self, params: Parameters<GetErrorsParams>) -> String {
        let id = match self.resolve_project_id(&params.0.name).await {
            Ok(id) => id,
            Err(e) => return e,
        };
        let lines = params.0.lines.unwrap_or(20);
        match reqwest::get(&format!("{}/projects/{}/errors?lines={}", self.api_url, id, lines)).await {
            Ok(resp) => match resp.text().await {
                Ok(body) => {
                    if body == "[]" {
                        format!("No recent errors for '{}'.", params.0.name)
                    } else {
                        body
                    }
                }
                Err(e) => format!("Error: {}", e),
            },
            Err(_) => "havn server not running.".to_string(),
        }
    }

    /// Find the nearest available (free) TCP port starting from a preferred port.
    /// Use this before starting a new dev server to avoid port conflicts.
    #[tool(
        name = "find_available_port",
        description = "Find the nearest free TCP port from a preferred starting port. Use before starting a dev server to avoid port conflicts."
    )]
    async fn find_available_port(&self, params: Parameters<FindAvailablePortParams>) -> String {
        let preferred = params.0.preferred.unwrap_or(3000);
        match reqwest::get(&format!("{}/available-port?preferred={}", self.api_url, preferred)).await {
            Ok(resp) => match resp.text().await {
                Ok(body) => body,
                Err(e) => format!("Error: {}", e),
            },
            Err(_) => "havn server not running.".to_string(),
        }
    }

    /// Get a full system overview: all projects, their health, running state, and error counts.
    /// Call this at the start of a session to understand the developer's current environment.
    #[tool(
        name = "get_system_overview",
        description = "Get a full overview of all projects: running state, ports, frameworks, uptime, and recent error counts. Use at the start of a session to understand the dev environment."
    )]
    async fn get_system_overview(&self, _params: Parameters<SystemOverviewParams>) -> String {
        match reqwest::get(&format!("{}/system-overview", self.api_url)).await {
            Ok(resp) => match resp.text().await {
                Ok(body) => body,
                Err(e) => format!("Error: {}", e),
            },
            Err(_) => "havn server not running. Start with `havn` or `havn install-service`.".to_string(),
        }
    }

    /// Get the effective environment variables a project would launch with.
    /// Shows the merged result of .env files, global secrets, and project-scoped secrets,
    /// with the source of each variable clearly labeled.
    #[tool(
        name = "get_effective_env",
        description = "Get the effective environment variables for a project (merged .env files + global secrets + project secrets). Shows the source of each variable."
    )]
    async fn get_effective_env(&self, params: Parameters<GetEffectiveEnvParams>) -> String {
        let id = match self.resolve_project_id(&params.0.name).await {
            Ok(id) => id,
            Err(e) => return e,
        };
        match reqwest::get(&format!("{}/projects/{}/effective-env", self.api_url, id)).await {
            Ok(resp) => match resp.text().await {
                Ok(body) => {
                    if body == "[]" {
                        format!("No environment variables configured for '{}'.", params.0.name)
                    } else {
                        body
                    }
                }
                Err(e) => format!("Error: {}", e),
            },
            Err(_) => "havn server not running.".to_string(),
        }
    }

    /// Get the current havn version and check if an update is available.
    #[tool(
        name = "get_version",
        description = "Get the current havn version and check if an update is available. Use at the start of a session to verify havn is current."
    )]
    async fn get_version(&self, _params: Parameters<GetVersionParams>) -> String {
        let current = env!("CARGO_PKG_VERSION");

        let url = "https://api.github.com/repos/Morrigan01/havn/releases/latest";
        let latest = reqwest::Client::builder()
            .user_agent("havn-mcp")
            .timeout(std::time::Duration::from_secs(3))
            .build()
            .ok()
            .and_then(|c| futures::executor::block_on(async {
                let resp = c.get(url).send().await.ok()?;
                let data: serde_json::Value = resp.json().await.ok()?;
                data.get("tag_name")
                    .and_then(|t| t.as_str())
                    .map(|t| t.strip_prefix('v').unwrap_or(t).to_string())
            }));

        match latest {
            Some(ref ver) if ver != current => {
                format!(
                    "havn v{} (update available: v{}). Tell the user to run `havn update` to upgrade.",
                    current, ver
                )
            }
            _ => format!("havn v{} (latest)", current),
        }
    }

    // ── Agent Helper Tools ─────────────────────────────────────────────────────

    /// Get recent stdout and stderr logs for a project.
    /// Use after restarting a service to verify your code fix worked,
    /// or to see what a service is printing.
    #[tool(
        name = "get_logs",
        description = "Get recent stdout/stderr logs for a project. Use after code changes and restart to verify the fix worked. Filter by stream (stdout, stderr, or all)."
    )]
    async fn get_logs(&self, params: Parameters<GetLogsParams>) -> String {
        let id = match self.resolve_project_id(&params.0.name).await {
            Ok(id) => id,
            Err(e) => return e,
        };
        let lines = params.0.lines.unwrap_or(50).min(500);
        let url = format!("{}/projects/{}/logs?lines={}", self.api_url, id, lines);
        match reqwest::get(&url).await {
            Ok(resp) => {
                let body = resp.text().await.unwrap_or_default();
                // Filter by stream if requested
                let stream_filter = params.0.stream.as_deref().unwrap_or("all");
                if stream_filter == "all" {
                    return body;
                }
                // Parse and filter
                if let Ok(logs) = serde_json::from_str::<Vec<serde_json::Value>>(&body) {
                    let filtered: Vec<&serde_json::Value> = logs.iter()
                        .filter(|l| l.get("stream").and_then(|s| s.as_str()) == Some(stream_filter))
                        .collect();
                    if filtered.is_empty() {
                        return format!("No {} logs for '{}'.", stream_filter, params.0.name);
                    }
                    serde_json::to_string_pretty(&filtered).unwrap_or(body)
                } else {
                    body
                }
            }
            Err(_) => "havn server not running.".to_string(),
        }
    }

    /// Run a shell command in a project's directory.
    /// Use for: npm install, cargo build, npx prisma migrate, pip install, etc.
    /// The command runs with the project's root as the working directory.
    #[tool(
        name = "run_command",
        description = "Run a shell command in a project's directory. Use for npm install, cargo build, migrations, linting, or any command that needs to run in the right directory. Returns stdout, stderr, and exit code."
    )]
    async fn run_command(&self, params: Parameters<RunCommandParams>) -> String {
        let id = match self.resolve_project_id(&params.0.name).await {
            Ok(id) => id,
            Err(e) => return e,
        };
        let timeout = params.0.timeout_secs.unwrap_or(30);
        let client = reqwest::Client::new();
        let body = serde_json::json!({
            "command": params.0.command,
            "timeout_secs": timeout,
        });
        match client
            .post(format!("{}/projects/{}/run", self.api_url, id))
            .json(&body)
            .timeout(std::time::Duration::from_secs(timeout + 5)) // HTTP timeout slightly longer
            .send()
            .await
        {
            Ok(resp) => resp.text().await.unwrap_or_default(),
            Err(e) if e.is_timeout() => {
                format!("Command timed out after {}s.", timeout)
            }
            Err(_) => "havn server not running.".to_string(),
        }
    }

    /// Check if a service is responding on a given port.
    /// Use after restarting a service to verify it's actually healthy,
    /// or to check if a dependency (database, API) is reachable.
    #[tool(
        name = "health_check",
        description = "Check if a service is responding on a port. Returns HTTP status, latency, and whether it's healthy. Use to verify a service is up after restart."
    )]
    async fn health_check(&self, params: Parameters<HealthCheckParams>) -> String {
        let mut url = format!("{}/health/{}", self.api_url, params.0.port);
        if let Some(ref path) = params.0.path {
            url = format!("{}?path={}", url, path);
        }
        match reqwest::get(&url).await {
            Ok(resp) => resp.text().await.unwrap_or_default(),
            Err(_) => "havn server not running.".to_string(),
        }
    }

    // ── Environment Awareness Tools ────────────────────────────────────────

    /// List all running Docker containers with their port mappings.
    /// Use to check if databases (Postgres, Redis, MongoDB) or other
    /// infrastructure services are running in Docker.
    #[tool(
        name = "docker_status",
        description = "List running Docker containers with port mappings. Use to check if databases (Postgres, Redis, MongoDB) or infrastructure services are running."
    )]
    async fn docker_status(&self, _params: Parameters<DockerStatusParams>) -> String {
        match reqwest::get(&format!("{}/docker", self.api_url)).await {
            Ok(resp) => resp.text().await.unwrap_or_default(),
            Err(_) => "havn server not running.".to_string(),
        }
    }

    /// Get CPU and memory usage for a project's processes.
    /// Use when the developer says their machine is slow, or to check
    /// if a service is consuming excessive resources.
    #[tool(
        name = "get_resources",
        description = "Get CPU and memory usage for a project's processes. Use when the machine is slow or to identify resource hogs."
    )]
    async fn get_resources(&self, params: Parameters<GetResourcesParams>) -> String {
        let id = match self.resolve_project_id(&params.0.name).await {
            Ok(id) => id,
            Err(e) => return e,
        };
        match reqwest::get(&format!("{}/projects/{}/resources", self.api_url, id)).await {
            Ok(resp) => resp.text().await.unwrap_or_default(),
            Err(_) => "havn server not running.".to_string(),
        }
    }

    /// Check if a project's dependencies are up to date.
    /// Compares lock file timestamps against installed dependencies.
    /// Detects stale node_modules, outdated cargo builds, and missing virtualenvs.
    /// Returns fix commands when deps need updating.
    #[tool(
        name = "check_deps",
        description = "Check if dependencies are up to date (node_modules, cargo build, pip venv). Returns fix commands if stale. Use before starting a service that might fail due to missing deps."
    )]
    async fn check_deps(&self, params: Parameters<CheckDepsParams>) -> String {
        let id = match self.resolve_project_id(&params.0.name).await {
            Ok(id) => id,
            Err(e) => return e,
        };
        match reqwest::get(&format!("{}/projects/{}/deps", self.api_url, id)).await {
            Ok(resp) => resp.text().await.unwrap_or_default(),
            Err(_) => "havn server not running.".to_string(),
        }
    }

    /// Check database connectivity for a project.
    /// Finds database URLs from env vars and .env files, tests TCP connectivity,
    /// and detects database containers running in Docker.
    /// Use when a service fails with "connection refused" or database errors.
    #[tool(
        name = "db_status",
        description = "Check database connectivity for a project. Finds DB URLs from env vars, tests connectivity, detects Docker DB containers. Use when a service has database connection errors."
    )]
    async fn db_status(&self, params: Parameters<DbStatusParams>) -> String {
        let id = match self.resolve_project_id(&params.0.name).await {
            Ok(id) => id,
            Err(e) => return e,
        };
        match reqwest::get(&format!("{}/projects/{}/db-status", self.api_url, id)).await {
            Ok(resp) => resp.text().await.unwrap_or_default(),
            Err(_) => "havn server not running.".to_string(),
        }
    }

    // ── Stack Orchestration MCP Tools ────────────────────────────────────────

    /// List all defined stacks (profiles) with their projects and running status.
    #[tool(
        name = "list_stacks",
        description = "List all defined stacks with their projects and running status. A stack is a named group of projects that can be started/stopped together."
    )]
    async fn list_stacks(&self, _params: Parameters<ListStacksParams>) -> String {
        match reqwest::get(&format!("{}/profiles", self.api_url)).await {
            Ok(resp) => resp.text().await.unwrap_or_default(),
            Err(_) => "havn server not running.".to_string(),
        }
    }

    /// Get detailed status of a stack: per-project running state, ports, and dependency edges.
    #[tool(
        name = "get_stack",
        description = "Get detailed status of a stack: which projects are running, healthy, or stopped, their ports, and the dependency graph."
    )]
    async fn get_stack(&self, params: Parameters<GetStackParams>) -> String {
        let id = match self.resolve_profile_id(&params.0.name).await {
            Ok(id) => id,
            Err(e) => return e,
        };
        match reqwest::get(&format!("{}/profiles/{}/detail", self.api_url, id)).await {
            Ok(resp) => resp.text().await.unwrap_or_default(),
            Err(_) => "havn server not running.".to_string(),
        }
    }

    /// Start all projects in a stack in dependency order, inject environment variables,
    /// and verify each service becomes healthy before starting the next.
    /// Returns per-service status: healthy, failed, or skipped (if a dependency failed).
    /// This is a blocking call — may take up to 180 seconds for large stacks.
    #[tool(
        name = "start_stack",
        description = "Start all projects in a stack in dependency order, verify each is healthy. Returns per-service status. Use this to bring up a full dev environment. May take up to 180s."
    )]
    async fn start_stack(&self, params: Parameters<StartStackParams>) -> String {
        let id = match self.resolve_profile_id(&params.0.name).await {
            Ok(id) => id,
            Err(e) => return e,
        };
        let client = reqwest::Client::new();
        match client
            .post(format!("{}/profiles/{}/start-stack", self.api_url, id))
            .timeout(std::time::Duration::from_secs(180))
            .send()
            .await
        {
            Ok(resp) => resp.text().await.unwrap_or_default(),
            Err(e) if e.is_timeout() => {
                "Stack startup timed out after 180s. Some services may still be starting. Use get_stack to check status.".to_string()
            }
            Err(_) => "havn server not running.".to_string(),
        }
    }

    /// Gracefully stop all projects in a stack in reverse dependency order.
    /// Skips projects that are shared with other running stacks.
    #[tool(
        name = "stop_stack",
        description = "Stop all projects in a stack in reverse dependency order. Skips shared dependencies still needed by other stacks."
    )]
    async fn stop_stack(&self, params: Parameters<StopStackParams>) -> String {
        let id = match self.resolve_profile_id(&params.0.name).await {
            Ok(id) => id,
            Err(e) => return e,
        };
        let client = reqwest::Client::new();
        match client
            .post(format!("{}/profiles/{}/stop-stack", self.api_url, id))
            .timeout(std::time::Duration::from_secs(60))
            .send()
            .await
        {
            Ok(resp) => resp.text().await.unwrap_or_default(),
            Err(_) => "havn server not running.".to_string(),
        }
    }

    /// Trace failures across a stack: identify root cause services, affected dependents,
    /// and suggest recovery actions.
    #[tool(
        name = "diagnose_stack",
        description = "Diagnose a stack: find which service crashed (root cause), which are affected (cascading failure), and get a recovery suggestion. Use when something in the stack is broken."
    )]
    async fn diagnose_stack(&self, params: Parameters<DiagnoseStackParams>) -> String {
        let id = match self.resolve_profile_id(&params.0.name).await {
            Ok(id) => id,
            Err(e) => return e,
        };
        match reqwest::get(&format!("{}/profiles/{}/diagnose", self.api_url, id)).await {
            Ok(resp) => resp.text().await.unwrap_or_default(),
            Err(_) => "havn server not running.".to_string(),
        }
    }

    /// Check for environment misconfigurations before starting a stack:
    /// port conflicts, missing env vars, and projects without start commands.
    #[tool(
        name = "validate_env",
        description = "Pre-flight check for a stack: detect port conflicts, missing environment variables, and projects without start commands. Run this before start_stack to catch issues early."
    )]
    async fn validate_env(&self, params: Parameters<ValidateEnvParams>) -> String {
        let id = match self.resolve_profile_id(&params.0.name).await {
            Ok(id) => id,
            Err(e) => return e,
        };
        match reqwest::get(&format!("{}/profiles/{}/validate-env", self.api_url, id)).await {
            Ok(resp) => resp.text().await.unwrap_or_default(),
            Err(_) => "havn server not running.".to_string(),
        }
    }
}

impl McpServer {
    /// Helper: resolve a project name to its ID.
    async fn resolve_project_id(&self, name: &str) -> Result<i64, String> {
        let projects_resp = reqwest::get(&format!("{}/projects", self.api_url))
            .await
            .map_err(|_| "havn server not running.".to_string())?;

        let projects: Vec<serde_json::Value> = projects_resp
            .json()
            .await
            .map_err(|e| format!("Error: {}", e))?;

        projects
            .iter()
            .find(|p| p.get("name").and_then(|n| n.as_str()) == Some(name))
            .and_then(|p| p.get("id").and_then(|i| i.as_i64()))
            .ok_or_else(|| format!("Project '{}' not found.", name))
    }

    /// Helper: resolve a stack (profile) name to its ID.
    async fn resolve_profile_id(&self, name: &str) -> Result<i64, String> {
        let profiles_resp = reqwest::get(&format!("{}/profiles", self.api_url))
            .await
            .map_err(|_| "havn server not running.".to_string())?;

        let profiles: Vec<serde_json::Value> = profiles_resp
            .json()
            .await
            .map_err(|e| format!("Error: {}", e))?;

        profiles
            .iter()
            .find(|p| p.get("name").and_then(|n| n.as_str()) == Some(name))
            .and_then(|p| p.get("id").and_then(|i| i.as_i64()))
            .ok_or_else(|| format!("Stack '{}' not found.", name))
    }
}

/// Start the MCP server on stdio transport.
pub async fn run(api_url: String) {
    let server = McpServer::new(api_url);

    let transport = rmcp::transport::io::stdio();

    let service = server.serve(transport).await;
    match service {
        Ok(running) => {
            let _ = running.waiting().await;
        }
        Err(e) => {
            eprintln!("MCP server error: {}", e);
            std::process::exit(1);
        }
    }
}
