use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "havn",
    about = "The MCP server that gives AI agents eyes and hands on your local dev environment",
    long_about = "havn scans your local ports, maps them to project directories, detects frameworks,\n\
                   and exposes 24 MCP tools for AI agents to see and control your dev environment.\n\n\
                   Quick start:\n  \
                   havn              Start dashboard at localhost:9390\n  \
                   havn status       See what's running\n  \
                   havn mcp          Start MCP server for AI tools\n  \
                   havn tools        List all 24 MCP tools\n  \
                   havn update       Self-update to latest release",
    version
)]
pub struct Cli {
    /// Port for the dashboard server
    #[arg(short, long, default_value = "9390")]
    pub port: u16,

    /// Address to bind to
    #[arg(short, long, default_value = "127.0.0.1")]
    pub bind: String,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Show all projects and ports
    Status,
    /// Kill a process by port number or project name
    Kill {
        /// Port number or project name
        target: String,
    },
    /// Register a project directory
    Add {
        /// Path to project directory
        path: PathBuf,
    },
    /// Unregister a project
    Remove {
        /// Project name or path
        target: String,
    },
    /// Get or set configuration
    Config {
        /// Configuration key
        key: String,
        /// Configuration value (omit to read)
        value: Option<String>,
    },
    /// Tail the server log file
    Logs,
    /// Start MCP server (stdio transport for AI tools)
    Mcp,
    /// Install as a system service (launchd/systemd)
    InstallService,
    /// Kill and restart a project using its configured start command
    Restart {
        /// Project name or path
        target: String,
    },
    /// Set the start command for a project (used by `restart`)
    SetStartCmd {
        /// Project name or path
        project: String,
        /// Shell command to start the project, e.g. "npm run dev"
        cmd: String,
    },
    /// Manage encrypted local secrets
    Secret {
        #[command(subcommand)]
        action: SecretAction,
    },
    /// Check for updates and self-update to the latest release
    Update,
    /// List all MCP tools available to AI agents
    Tools,
}

#[derive(Subcommand)]
pub enum SecretAction {
    /// Store an encrypted secret
    Set {
        /// Secret key
        key: String,
        /// Secret value
        value: String,
        /// Scope to a project (omit for global)
        #[arg(short, long)]
        project: Option<String>,
    },
    /// Retrieve a secret value
    Get {
        /// Secret key
        key: String,
        /// Project scope (omit for global)
        #[arg(short, long)]
        project: Option<String>,
    },
    /// List secret keys (values are not shown)
    List {
        /// Project scope (omit for global)
        #[arg(short, long)]
        project: Option<String>,
    },
    /// Delete a secret
    Delete {
        /// Secret key
        key: String,
        /// Project scope (omit for global)
        #[arg(short, long)]
        project: Option<String>,
    },
}

pub async fn status(args: &Cli) {
    let url = format!("http://{}:{}/projects", args.bind, args.port);
    match reqwest::get(&url).await {
        Ok(resp) => match resp.json::<Vec<crate::api::ProjectResponse>>().await {
            Ok(projects) => {
                if projects.is_empty() {
                    println!("No projects detected. Start a dev server and run again.");
                    return;
                }
                println!(
                    "{:<20} {:<12} {:<15} {:<10}",
                    "PROJECT", "FRAMEWORK", "PORTS", "UPTIME"
                );
                println!("{}", "-".repeat(60));
                for p in &projects {
                    let ports: Vec<String> = p.ports.iter().map(|p| format!(":{}", p)).collect();
                    let framework = p.framework.as_deref().unwrap_or("-");
                    println!(
                        "{}{:<20} {:<12} {:<15} {}",
                        if p.favorite { "★ " } else { "  " },
                        p.name,
                        framework,
                        ports.join(" "),
                        format_uptime(p.uptime_seconds),
                    );
                }
            }
            Err(e) => eprintln!("Failed to parse response: {}", e),
        },
        Err(_) => {
            println!("Server not running. Performing one-shot scan...");
            one_shot_scan().await;
        }
    }
}

async fn one_shot_scan() {
    let results = crate::scanner::scan_once().await;
    if results.is_empty() {
        println!("No listening ports detected.");
        return;
    }
    println!(
        "{:<20} {:<12} {:<10}",
        "PROJECT", "FRAMEWORK", "PORT"
    );
    println!("{}", "-".repeat(45));
    for entry in &results {
        let name = entry
            .project_name
            .as_deref()
            .unwrap_or("(unknown)");
        let framework = entry.framework.as_deref().unwrap_or("-");
        println!("{:<20} {:<12} :{}", name, framework, entry.port);
    }
}

pub async fn kill(args: &Cli, target: &str) {
    let url = if let Ok(port) = target.parse::<u16>() {
        format!("http://{}:{}/kill/{}", args.bind, args.port, port)
    } else {
        // Try to find project by name first
        let projects_url = format!("http://{}:{}/projects", args.bind, args.port);
        match reqwest::get(&projects_url).await {
            Ok(resp) => {
                if let Ok(projects) = resp.json::<Vec<crate::api::ProjectResponse>>().await {
                    if let Some(p) = projects.iter().find(|p| p.name == target) {
                        format!(
                            "http://{}:{}/projects/{}/kill",
                            args.bind, args.port, p.id
                        )
                    } else {
                        eprintln!("Project '{}' not found.", target);
                        return;
                    }
                } else {
                    eprintln!("Failed to fetch projects.");
                    return;
                }
            }
            Err(_) => {
                eprintln!("Server not running. Start with `havn` first.");
                return;
            }
        }
    };

    let client = reqwest::Client::new();
    match client.post(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            println!("Killed: {}", target);
        }
        Ok(resp) => {
            let body = resp.text().await.unwrap_or_default();
            eprintln!("Kill failed: {}", body);
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}

pub async fn add(args: &Cli, path: &std::path::Path) {
    let abs_path = std::fs::canonicalize(path).unwrap_or_else(|_| {
        eprintln!("Path does not exist: {}", path.display());
        std::process::exit(1);
    });

    let url = format!("http://{}:{}/projects", args.bind, args.port);
    let client = reqwest::Client::new();
    let body = serde_json::json!({ "path": abs_path.to_string_lossy() });
    match client.post(&url).json(&body).send().await {
        Ok(resp) if resp.status().is_success() => {
            println!("Added: {}", abs_path.display());
        }
        Ok(resp) => {
            let body = resp.text().await.unwrap_or_default();
            eprintln!("Failed: {}", body);
        }
        Err(_) => eprintln!("Server not running. Start with `havn` first."),
    }
}

pub async fn remove(args: &Cli, target: &str) {
    let projects_url = format!("http://{}:{}/projects", args.bind, args.port);
    let resp = match reqwest::get(&projects_url).await {
        Ok(r) => r,
        Err(_) => {
            eprintln!("Server not running. Start with `havn` first.");
            return;
        }
    };

    let projects: Vec<crate::api::ProjectResponse> = match resp.json().await {
        Ok(p) => p,
        Err(_) => {
            eprintln!("Failed to parse response.");
            return;
        }
    };

    let project = match projects.iter().find(|p| p.name == target || p.path == target) {
        Some(p) => p,
        None => {
            eprintln!("Project '{}' not found.", target);
            return;
        }
    };

    let url = format!("http://{}:{}/projects/{}", args.bind, args.port, project.id);
    let client = reqwest::Client::new();
    let del_resp = client.delete(&url).send().await;
    match del_resp {
        Ok(r) if r.status().is_success() => println!("Removed: {}", target),
        _ => eprintln!("Failed to remove project."),
    }
}

pub fn config_cmd(key: &str, value: Option<&str>) {
    let config = crate::config::Config::load();
    if let Some(val) = value {
        let mut config = config;
        match key {
            "dashboard_port" => {
                config.dashboard_port = val.parse().expect("Invalid port number");
            }
            "scan_interval" => {
                config.scan_interval_secs = val.parse().expect("Invalid interval");
            }
            "log_level" => {
                config.log_level = val.to_string();
            }
            _ => {
                eprintln!("Unknown config key: {}", key);
                return;
            }
        }
        config.save();
        println!("Set {} = {}", key, val);
    } else {
        match key {
            "dashboard_port" => println!("{}", config.dashboard_port),
            "scan_interval" => println!("{}", config.scan_interval_secs),
            "log_level" => println!("{}", config.log_level),
            _ => eprintln!("Unknown config key: {}", key),
        }
    }
}

pub async fn logs() {
    let log_path = crate::config::log_file_path();
    if !log_path.exists() {
        eprintln!("No log file found at {}", log_path.display());
        return;
    }
    let output = tokio::process::Command::new("tail")
        .args(["-f", &log_path.to_string_lossy()])
        .status()
        .await;
    if let Err(e) = output {
        eprintln!("Failed to tail logs: {}", e);
    }
}

pub async fn mcp(args: &Cli) {
    let api_url = format!("http://{}:{}", args.bind, args.port);
    crate::mcp::run(api_url).await;
}

pub fn install_service() {
    crate::service::install();
}

pub async fn restart(args: &Cli, target: &str) {
    let projects_url = format!("http://{}:{}/projects", args.bind, args.port);
    let resp = match reqwest::get(&projects_url).await {
        Ok(r) => r,
        Err(_) => {
            eprintln!("Server not running. Start with `havn` first.");
            return;
        }
    };

    let projects: Vec<crate::api::ProjectResponse> = match resp.json().await {
        Ok(p) => p,
        Err(_) => {
            eprintln!("Failed to parse response.");
            return;
        }
    };

    let project = match projects.iter().find(|p| p.name == target || p.path == target) {
        Some(p) => p,
        None => {
            eprintln!("Project '{}' not found.", target);
            return;
        }
    };

    let url = format!("http://{}:{}/projects/{}/restart", args.bind, args.port, project.id);
    let client = reqwest::Client::new();
    match client.post(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            println!("Restarting: {}", target);
        }
        Ok(resp) => {
            let body = resp.json::<serde_json::Value>().await.unwrap_or_default();
            eprintln!("Restart failed: {}", body.get("message").and_then(|m| m.as_str()).unwrap_or("unknown error"));
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}

pub async fn set_start_cmd(args: &Cli, project: &str, cmd: &str) {
    let projects_url = format!("http://{}:{}/projects", args.bind, args.port);
    let resp = match reqwest::get(&projects_url).await {
        Ok(r) => r,
        Err(_) => {
            eprintln!("Server not running. Start with `havn` first.");
            return;
        }
    };

    let projects: Vec<crate::api::ProjectResponse> = match resp.json().await {
        Ok(p) => p,
        Err(_) => {
            eprintln!("Failed to parse response.");
            return;
        }
    };

    let found = match projects.iter().find(|p| p.name == project || p.path == project) {
        Some(p) => p,
        None => {
            eprintln!("Project '{}' not found.", project);
            return;
        }
    };

    let url = format!("http://{}:{}/projects/{}", args.bind, args.port, found.id);
    let client = reqwest::Client::new();
    let body = serde_json::json!({ "start_cmd": cmd });
    match client.patch(&url).json(&body).send().await {
        Ok(resp) if resp.status().is_success() => {
            println!("Set start command for '{}': {}", project, cmd);
        }
        _ => eprintln!("Failed to set start command."),
    }
}

pub async fn secret(args: &Cli, action: &SecretAction) {
    let base = format!("http://{}:{}", args.bind, args.port);
    let client = reqwest::Client::new();

    match action {
        SecretAction::Set { key, value, project } => {
            let mut body = serde_json::json!({ "key": key, "value": value });
            if let Some(p) = project {
                body["project"] = serde_json::json!(p);
            }
            match client.post(format!("{}/secrets", base)).json(&body).send().await {
                Ok(r) if r.status().is_success() => println!("Secret '{}' stored.", key),
                Ok(r) => eprintln!("Failed: {}", r.text().await.unwrap_or_default()),
                Err(e) => eprintln!("Error: {}", e),
            }
        }
        SecretAction::Get { key, project } => {
            let mut url = format!("{}/secrets/{}", base, key);
            if let Some(p) = project {
                url = format!("{}?project={}", url, p);
            }
            match client.get(&url).send().await {
                Ok(r) if r.status().is_success() => {
                    let data: serde_json::Value = r.json().await.unwrap_or_default();
                    println!("{}", data.get("value").and_then(|v| v.as_str()).unwrap_or(""));
                }
                Ok(r) if r.status() == 404 => eprintln!("Secret '{}' not found.", key),
                Ok(r) => eprintln!("Failed: {}", r.text().await.unwrap_or_default()),
                Err(e) => eprintln!("Error: {}", e),
            }
        }
        SecretAction::List { project } => {
            let mut url = format!("{}/secrets", base);
            if let Some(p) = project {
                url = format!("{}?project={}", url, p);
            }
            match client.get(&url).send().await {
                Ok(r) if r.status().is_success() => {
                    let keys: Vec<String> = r.json().await.unwrap_or_default();
                    if keys.is_empty() {
                        println!("No secrets stored.");
                    } else {
                        for k in &keys {
                            println!("{}", k);
                        }
                    }
                }
                Ok(r) => eprintln!("Failed: {}", r.text().await.unwrap_or_default()),
                Err(e) => eprintln!("Error: {}", e),
            }
        }
        SecretAction::Delete { key, project } => {
            let mut url = format!("{}/secrets/{}", base, key);
            if let Some(p) = project {
                url = format!("{}?project={}", url, p);
            }
            match client.delete(&url).send().await {
                Ok(r) if r.status().is_success() => println!("Deleted secret '{}'.", key),
                Ok(r) if r.status() == 404 => eprintln!("Secret '{}' not found.", key),
                Ok(r) => eprintln!("Failed: {}", r.text().await.unwrap_or_default()),
                Err(e) => eprintln!("Error: {}", e),
            }
        }
    }
}

pub fn list_tools() {
    println!("havn MCP Tools (24 total)");
    println!("=========================");
    println!("Configure in Claude Code / Cursor: {{ \"mcpServers\": {{ \"havn\": {{ \"command\": \"havn\", \"args\": [\"mcp\"] }} }} }}");
    println!();
    println!("DISCOVERY");
    println!("  list_projects        List all running projects with ports and frameworks");
    println!("  get_project          Get details about a specific project");
    println!("  get_system_overview  Full snapshot of your dev environment (start here)");
    println!("  get_version          Check havn version and available updates");
    println!();
    println!("PROCESS CONTROL");
    println!("  kill_port            Kill the process on a specific port");
    println!("  restart_and_verify   Restart a project and wait until healthy");
    println!("  run_command          Run a shell command in a project's directory");
    println!();
    println!("STACK ORCHESTRATION");
    println!("  list_stacks          List all defined stacks with running status");
    println!("  get_stack            Detailed stack status with dependency graph");
    println!("  start_stack          Start services in dependency order, verify health");
    println!("  stop_stack           Graceful shutdown in reverse dependency order");
    println!();
    println!("DEBUGGING");
    println!("  get_errors           Recent stderr, panics, and exceptions for a project");
    println!("  get_logs             Stdout/stderr logs (verify fixes after restart)");
    println!("  diagnose_stack       Find root cause of failures across services");
    println!("  health_check         Check if a port is responding (HTTP status + latency)");
    println!();
    println!("ENVIRONMENT");
    println!("  get_effective_env    Merged env vars (.env + secrets) for a project");
    println!("  validate_env         Pre-flight check: port conflicts, missing vars");
    println!("  check_deps           Dependency freshness (stale node_modules, etc.)");
    println!("  db_status            Database connectivity + Docker DB detection");
    println!();
    println!("INFRASTRUCTURE");
    println!("  docker_status        Running Docker containers with port mappings");
    println!("  get_resources        CPU and memory usage per project");
    println!();
    println!("SECRETS");
    println!("  list_secrets         List secret keys (values hidden)");
    println!("  get_secret           Retrieve a decrypted secret");
    println!("  set_secret           Store an encrypted secret (AES-256-GCM)");
    println!();
    println!("UTILITY");
    println!("  find_port_for_project  Look up which port(s) a project uses");
    println!("  find_available_port    Find a free TCP port");
}

const GITHUB_REPO: &str = "Morrigan01/havn";

/// Check GitHub for the latest release. Returns (latest_version, download_url) if newer.
async fn check_latest_release() -> Option<(String, String)> {
    let url = format!("https://api.github.com/repos/{}/releases/latest", GITHUB_REPO);
    let client = reqwest::Client::builder()
        .user_agent("havn-update-checker")
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .ok()?;

    let resp = client.get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let data: serde_json::Value = resp.json().await.ok()?;
    let latest_tag = data.get("tag_name")?.as_str()?;
    let latest_version = latest_tag.strip_prefix('v').unwrap_or(latest_tag);

    let current = env!("CARGO_PKG_VERSION");
    if latest_version != current && version_is_newer(latest_version, current) {
        // Find the right asset for this platform
        let target = current_target();
        let download_url = data.get("assets")
            .and_then(|a| a.as_array())
            .and_then(|assets| {
                assets.iter().find_map(|asset| {
                    let name = asset.get("name")?.as_str()?;
                    if name.contains(&target) {
                        asset.get("browser_download_url")?.as_str().map(|s| s.to_string())
                    } else {
                        None
                    }
                })
            })
            .unwrap_or_else(|| {
                format!("https://github.com/{}/releases/tag/{}", GITHUB_REPO, latest_tag)
            });

        Some((latest_version.to_string(), download_url))
    } else {
        None
    }
}

fn version_is_newer(latest: &str, current: &str) -> bool {
    let parse = |v: &str| -> Vec<u32> {
        v.split('.').filter_map(|s| s.parse().ok()).collect()
    };
    let l = parse(latest);
    let c = parse(current);
    l > c
}

fn current_target() -> String {
    let arch = if cfg!(target_arch = "aarch64") { "aarch64" } else { "x86_64" };
    let os = if cfg!(target_os = "macos") { "apple-darwin" } else { "unknown-linux-gnu" };
    format!("{}-{}", arch, os)
}

/// Print a one-line update notice on startup (non-blocking).
pub async fn check_for_update_notice() {
    // Run in background so it doesn't slow startup
    tokio::spawn(async {
        if let Some((version, _url)) = check_latest_release().await {
            eprintln!(
                "\n  Update available: v{} -> v{}. Run `havn update` to upgrade.\n",
                env!("CARGO_PKG_VERSION"),
                version
            );
        }
    });
}

/// Self-update: download the latest release binary and replace the current one.
pub async fn update() {
    println!("Checking for updates...");

    match check_latest_release().await {
        None => {
            println!("You're on the latest version (v{}).", env!("CARGO_PKG_VERSION"));
            return;
        }
        Some((version, url)) => {
            println!("New version available: v{} (current: v{})", version, env!("CARGO_PKG_VERSION"));

            if url.starts_with("https://github.com") && url.contains("/releases/tag/") {
                // No binary asset for this platform, point to releases page
                println!("No pre-built binary found for your platform ({}).", current_target());
                println!("Download manually: {}", url);
                println!("Or update from source: cargo install --path . --force");
                return;
            }

            println!("Downloading from: {}", url);

            let client = reqwest::Client::builder()
                .user_agent("havn-updater")
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .unwrap();

            let resp = match client.get(&url).send().await {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Download failed: {}", e);
                    return;
                }
            };

            if !resp.status().is_success() {
                eprintln!("Download failed: HTTP {}", resp.status());
                return;
            }

            let bytes = match resp.bytes().await {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("Download failed: {}", e);
                    return;
                }
            };

            // Find current binary path
            let current_exe = match std::env::current_exe() {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Cannot determine binary path: {}", e);
                    return;
                }
            };

            // Write to a temp file next to the binary, then atomically rename
            let tmp_path = current_exe.with_extension("tmp");
            if let Err(e) = std::fs::write(&tmp_path, &bytes) {
                eprintln!("Failed to write update: {}", e);
                return;
            }

            // Make executable
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o755)).ok();
            }

            // Replace the old binary
            let backup_path = current_exe.with_extension("old");
            if let Err(e) = std::fs::rename(&current_exe, &backup_path) {
                eprintln!("Failed to backup current binary: {}", e);
                std::fs::remove_file(&tmp_path).ok();
                return;
            }

            if let Err(e) = std::fs::rename(&tmp_path, &current_exe) {
                eprintln!("Failed to install update: {}", e);
                // Try to restore backup
                std::fs::rename(&backup_path, &current_exe).ok();
                return;
            }

            // Clean up backup
            std::fs::remove_file(&backup_path).ok();

            println!("Updated to v{}. Restart havn to use the new version.", version);
        }
    }
}

fn format_uptime(seconds: u64) -> String {
    if seconds < 60 {
        format!("{}s", seconds)
    } else if seconds < 3600 {
        format!("{}m", seconds / 60)
    } else {
        let h = seconds / 3600;
        let m = (seconds % 3600) / 60;
        format!("{}h {}m", h, m)
    }
}
