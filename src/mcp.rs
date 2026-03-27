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
            .post(&format!("{}/kill/{}", self.api_url, params.0.port))
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
