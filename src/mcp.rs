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
            Err(_) => "scanprojects server not running. Start with `scanprojects` or `scanprojects install-service`.".to_string(),
        }
    }

    /// Get details about a specific project by name.
    #[tool(name = "get_project", description = "Get details about a specific project")]
    async fn get_project(&self, params: Parameters<GetProjectParams>) -> String {
        let projects_resp = match reqwest::get(&format!("{}/projects", self.api_url)).await {
            Ok(r) => r,
            Err(_) => return "scanprojects server not running.".to_string(),
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
            Err(_) => "scanprojects server not running.".to_string(),
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
            Err(_) => return "scanprojects server not running.".to_string(),
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
            Err(_) => "scanprojects server not running.".to_string(),
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
            Err(_) => "scanprojects server not running.".to_string(),
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
            Err(_) => "scanprojects server not running.".to_string(),
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
            Err(_) => return "scanprojects server not running.".to_string(),
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
