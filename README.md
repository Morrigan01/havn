# havn

**The MCP server that gives AI agents eyes and hands on your local dev environment.**

> *havn* (Danish: harbor) — where all your dev servers dock.

```
Agent: Let me check what's running on your machine.
> havn.get_system_overview()

  PROJECT              FRAMEWORK    PORTS           STATUS
  ------------------------------------------------------------
  my-api             Express      :8080 :8081     healthy (2h 15m)
  frontend           Next.js      :3000           healthy (45m)
  worker-service     Rust         :9001           healthy (12m)
  admin-panel        Vite         :5173           3 errors

Agent: admin-panel has errors. Let me check.
> havn.get_errors({ name: "admin-panel" })

  [TypeError: Cannot read properties of undefined (reading 'user')]
  at Dashboard.tsx:42

Agent: Found it — a null-check issue. Let me fix that and restart.
> havn.restart_and_verify({ name: "admin-panel" })

  { status: "healthy", boot_time: "1.2s", port: 5173 }
```

## The problem

You're running 5+ dev servers. You can't remember which project is on which port. Your AI agent can't see any of it. You paste `lsof` output into chat. Again.

havn fixes this by continuously scanning your local ports, mapping them to project directories, detecting the framework, and exposing everything through an MCP server that any AI agent can use — plus a CLI and web dashboard for when you want to look yourself.

## MCP Server (for AI agents)

havn includes a built-in [MCP server](https://modelcontextprotocol.io/) so AI coding tools like Claude Code, Cursor, and Windsurf can query and control your dev environment directly.

### Configure in Claude Code

Add to your MCP settings:

```json
{
  "mcpServers": {
    "havn": {
      "command": "havn",
      "args": ["mcp"]
    }
  }
}
```

### Available tools

#### Project tools

| Tool | Description |
|------|-------------|
| `list_projects` | List all running projects and their ports |
| `get_project` | Get details about a specific project |
| `kill_port` | Kill the process on a specific port |
| `find_port_for_project` | Find which port(s) a project is running on |
| `restart_and_verify` | Restart a project and confirm it's healthy |
| `get_errors` | Get recent errors (stderr, panics, exceptions) for a project |
| `find_available_port` | Find the nearest free TCP port |
| `get_system_overview` | Full snapshot of all projects and their status |
| `get_effective_env` | Merged environment variables (.env + secrets) for a project |
| `list_secrets` / `get_secret` / `set_secret` | Manage encrypted secrets |

#### Stack orchestration tools

| Tool | Description |
|------|-------------|
| `list_stacks` | List all defined stacks and their current state |
| `get_stack` | Get full details of a stack: services, ports, health, dependencies |
| `start_stack` | Start all services in a stack in dependency order |
| `stop_stack` | Gracefully stop all services in a stack (reverse dependency order) |
| `diagnose_stack` | Run health checks across all stack services and report issues |
| `validate_env` | Check that all required environment variables are set before starting |

### Stack Orchestration

A stack is a group of projects that form a local dev environment — for example, an API server, a frontend, a database, and a worker. Agents can use the stack tools to bring up an entire environment in one shot.

```
Agent: Let me start the full-stack environment.
> havn.validate_env({ stack: "my-app" })

  { valid: true, services: ["postgres", "my-api", "frontend", "worker"] }

> havn.start_stack({ stack: "my-app" })

  Starting postgres...       :5432  healthy (0.8s)
  Starting my-api...         :8080  healthy (2.1s)
  Starting frontend...       :3000  healthy (1.4s)
  Starting worker...         :9001  healthy (0.6s)

  Stack "my-app" is running. 4/4 services healthy.

Agent: Your full dev stack is up. The frontend is at localhost:3000
       and the API is at localhost:8080.
```

The agent can also diagnose a broken stack without manual debugging:

```
Agent: The frontend is showing a blank page. Let me diagnose the stack.
> havn.diagnose_stack({ stack: "my-app" })

  postgres        :5432   healthy
  my-api          :8080   unhealthy — connection refused to postgres:5432
  frontend        :3000   healthy (but depends on my-api)
  worker          :9001   healthy

  Root cause: my-api cannot reach postgres. Check DATABASE_URL.
```

## Install

### From source (requires Rust)

```bash
cargo install --path .
```

### From GitHub releases

Download the binary for your platform from [Releases](https://github.com/omarelloumi/havn/releases).

## CLI commands

```bash
havn                 # Start the dashboard + background server
havn status          # Show all projects and ports
havn kill 3000       # Kill the process on port 3000
havn kill my-api     # Kill all processes for a project
havn add ~/my-project # Register a project manually
havn remove my-api   # Unregister a project
havn config key val  # Set configuration
havn logs            # Tail server logs
havn install-service # Install as launchd/systemd service
havn mcp             # Start MCP server for AI tools
```

### Run as a background service

```bash
havn install-service
# macOS: launchctl load ~/Library/LaunchAgents/com.havn.daemon.plist
# Linux: systemctl --user enable --now havn
```

## Dashboard

```bash
havn
```

Opens a web dashboard at `http://localhost:9390` showing all your running projects, their ports, frameworks, and uptime. Kill processes with one click.

## Framework detection

havn auto-detects frameworks by scanning project directories:

| Framework | Detection |
|-----------|-----------|
| Next.js | `package.json` with `next` dependency |
| Vite | `package.json` with `vite` dependency |
| Express | `package.json` with `express` dependency |
| React (CRA) | `package.json` with `react-scripts` dependency |
| Rust (web) | `Cargo.toml` with `axum`/`actix-web`/`rocket` |
| Go | `go.mod` present |
| Django | `manage.py` present |
| FastAPI | `pyproject.toml` with `fastapi` |
| Rails | `Gemfile` with `rails` |
| Docker Compose | `docker-compose.yml` present |

## Configuration

Config is stored in platform-standard directories:
- macOS: `~/Library/Application Support/havn/config.json`
- Linux: `~/.config/havn/config.json`

```bash
havn config dashboard_port 9390
havn config scan_interval 5
havn config log_level info
```

## How it works

1. **Scanner** polls `lsof` every 5 seconds to find listening TCP ports
2. **Project resolver** walks up from each process's working directory to find project roots (package.json, Cargo.toml, .git, etc.)
3. **Framework detector** parses project files to identify the framework
4. **Registry** (SQLite) persists project history across restarts
5. **Dashboard** shows everything in real-time via WebSocket
6. **MCP server** (rmcp) lets AI tools query and control your dev environment

## Security

havn is designed for **single-user local development machines**. Understanding its security model is important before using it.

### What's protected

- **Secrets at rest** — encrypted with AES-256-GCM. The master key is stored with `0600` permissions (owner-read-only).
- **CORS** — the API rejects cross-origin requests, preventing malicious websites from calling your local API.
- **Localhost binding** — the server binds to `127.0.0.1` by default, not exposed to the network.
- **Rate limiting** — destructive endpoints (kill, restart) are rate-limited to prevent abuse.
- **Command validation** — start commands are checked against dangerous shell patterns.

### Assumptions and limitations

- **No authentication** — anyone who can reach `localhost:9390` can use the API. This is safe on single-user machines but not on shared systems.
- **No HTTPS** — traffic is unencrypted. This is acceptable for localhost-only usage.
- **Start commands run as your user** — the `restart` feature executes shell commands with your permissions. Only set start commands you trust.
- **macOS optimized** — kqueue-based process monitoring is macOS-native. Linux falls back to polling.

### Recommendations

- Do **not** bind to `0.0.0.0` in untrusted environments (use the default `127.0.0.1`).
- If running on a shared machine, firewall port 9390 to your user.
- Review start commands before setting them via the dashboard.

## Contributing

Contributions are welcome! Please open an issue or pull request.

## License

MIT
