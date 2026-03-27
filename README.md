# scanprojects

**Map local ports to project directories.** CLI, dashboard, and MCP server for developers juggling multiple projects.

```
$ scanprojects status
PROJECT              FRAMEWORK    PORTS           UPTIME
------------------------------------------------------------
★ my-api             Express      :8080 :8081     2h 15m
★ frontend           Next.js      :3000           45m
  worker-service     Rust         :9001           12m
  admin-panel        Vite         :5173           5m
```

## The problem

You're running 5+ dev servers. You can't remember which project is on which port. You ask your AI agent to run `lsof` for you. Again.

scanprojects fixes this by continuously scanning your local ports, mapping them to project directories, detecting the framework, and giving you a dashboard to see and control everything.

## Install

### From source (requires Rust)

```bash
cargo install --path .
```

### From GitHub releases

Download the binary for your platform from [Releases](https://github.com/scanprojects/scanprojects/releases).

## Usage

### Start the dashboard

```bash
scanprojects
```

Opens a web dashboard at `http://localhost:9390` showing all your running projects, their ports, frameworks, and uptime. Kill processes with one click.

### CLI commands

```bash
scanprojects status          # Show all projects and ports
scanprojects kill 3000       # Kill the process on port 3000
scanprojects kill my-api     # Kill all processes for a project
scanprojects add ~/my-project # Register a project manually
scanprojects remove my-api   # Unregister a project
scanprojects config key val  # Set configuration
scanprojects logs            # Tail server logs
scanprojects install-service # Install as launchd/systemd service
scanprojects mcp             # Start MCP server for AI tools
```

### Run as a background service

```bash
scanprojects install-service
# macOS: launchctl load ~/Library/LaunchAgents/com.scanprojects.daemon.plist
# Linux: systemctl --user enable --now scanprojects
```

## MCP Server (for AI tools)

scanprojects includes an MCP server so AI coding tools like Claude Code and Cursor can query your dev environment.

### Configure in Claude Code

Add to your MCP settings:

```json
{
  "mcpServers": {
    "scanprojects": {
      "command": "scanprojects",
      "args": ["mcp"]
    }
  }
}
```

### Available tools

| Tool | Description |
|------|-------------|
| `list_projects` | List all running projects and their ports |
| `get_project` | Get details about a specific project |
| `kill_port` | Kill the process on a specific port |
| `find_port_for_project` | Find which port(s) a project is running on |

## Framework detection

scanprojects auto-detects frameworks by scanning project directories:

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
- macOS: `~/Library/Application Support/scanprojects/config.json`
- Linux: `~/.config/scanprojects/config.json`

```bash
scanprojects config dashboard_port 9390
scanprojects config scan_interval 5
scanprojects config log_level info
```

## How it works

1. **Scanner** polls `lsof` every 5 seconds to find listening TCP ports
2. **Project resolver** walks up from each process's working directory to find project roots (package.json, Cargo.toml, .git, etc.)
3. **Framework detector** parses project files to identify the framework
4. **Registry** (SQLite) persists project history across restarts
5. **Dashboard** (Preact SPA) shows everything in real-time via WebSocket
6. **MCP server** (rmcp) lets AI tools query and control your dev environment

## License

MIT
