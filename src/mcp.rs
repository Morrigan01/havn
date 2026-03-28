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
