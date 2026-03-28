<p align="center">
  <h1 align="center">havn</h1>
  <p align="center">
    <strong>The MCP server that gives AI agents eyes and hands on your local dev environment.</strong>
  </p>
  <p align="center">
    <a href="https://github.com/Morrigan01/havn/actions/workflows/ci.yml"><img src="https://github.com/Morrigan01/havn/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
    <a href="https://github.com/Morrigan01/havn/releases/latest"><img src="https://img.shields.io/github/v/release/Morrigan01/havn?label=release&color=blue" alt="Release"></a>
    <a href="LICENSE"><img src="https://img.shields.io/badge/license-BSL--1.1-orange" alt="License"></a>
    <a href="https://modelcontextprotocol.io/"><img src="https://img.shields.io/badge/MCP-compatible-green" alt="MCP"></a>
    <img src="https://img.shields.io/badge/platform-macOS%20%7C%20Linux-lightgrey" alt="Platform">
  </p>
</p>

> *havn* (Danish: harbor) ... where all your dev servers dock.

---

## Why havn exists

Modern developers run multiple dev servers at once: a frontend on `:3000`, an API on `:8080`, a database on `:5432`, a worker on `:9001`. You lose track of what's running where. Your AI coding agent (Claude Code, Cursor, Windsurf) can write code for you, but it has no idea what's happening on your machine. It can't see your running services, can't tell you which one crashed, and can't start your dev environment for you.

**havn is the missing link between your AI agent and your local dev environment.**

It's a single Rust binary that:
- **Scans** your machine every 5 seconds to discover running dev servers
- **Maps** each process to its project directory and detects the framework (Next.js, Express, Vite, Django, Rails, etc.)
- **Exposes** everything through an [MCP server](https://modelcontextprotocol.io/) so your AI agent can see, control, and orchestrate your local services
- **Provides** a web dashboard at `localhost:9390` and a CLI for when you want to look yourself

### Who it's for

- **Developers running 3+ concurrent services** who use AI coding agents
- **Anyone tired of pasting `lsof` output** into AI chat to explain what's running
- **Teams building microservices locally** who want one-command stack startup

### What you can do with it

| Use case | How |
|----------|-----|
| See what's running | `havn status` or ask your agent |
| Start your full dev stack | Agent calls `start_stack("payments")`, services start in dependency order |
| Debug a broken service | Agent calls `diagnose_stack`, finds root cause across services |
| Restart a crashed server | Agent calls `restart_and_verify`, confirms it's healthy |
| Check for env issues | Agent calls `validate_env` before starting, catches port conflicts and missing vars |
| Manage secrets | `havn secret set DB_PASSWORD ...` (AES-256-GCM encrypted) |
| Run as a background daemon | `havn install-service` (launchd on macOS, systemd on Linux) |

---

## See it in action

Your AI agent says one sentence. havn orchestrates the rest.

```
You:   "Start everything for the payments feature"

Agent: > havn.start_stack({ name: "payments" })

       Starting postgres...       :5432  healthy (0.8s)
       Starting api-server...     :8080  healthy (2.1s)
       Starting frontend...       :3000  healthy (1.4s)
       Starting worker...         :9001  healthy (0.6s)

       Stack "payments" is running. 4/4 services healthy.
       Frontend at localhost:3000, API at localhost:8080.
```

```
You:   "Why is the frontend showing a blank page?"

Agent: > havn.diagnose_stack({ name: "payments" })

       postgres    :5432   healthy
       api-server  :8080   crashed — ECONNREFUSED on :5432
       frontend    :3000   unhealthy — depends on api-server
       worker      :9001   healthy

       Root cause: api-server crashed. Suggestion: restart api-server.
```

---

## Quick start

### Install

```bash
# From source (requires Rust)
cargo install --path .

# Or download a binary from GitHub Releases
# https://github.com/Morrigan01/havn/releases
```

### Run

```bash
havn              # Start dashboard at localhost:9390
havn status       # See what's running
```

### Connect your AI agent

Add to your Claude Code / Cursor MCP settings:

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

That's it. Your agent can now see and control your local dev environment.

---

## MCP Tools

havn exposes **16 tools** through the [Model Context Protocol](https://modelcontextprotocol.io/). Works with Claude Code, Cursor, Windsurf, and any MCP-compatible client.

### Project tools

| Tool | What it does |
|------|-------------|
| `list_projects` | List all running projects with ports, frameworks, uptime |
| `get_project` | Get details about a specific project |
| `get_system_overview` | Full snapshot of your dev environment (start here) |
| `kill_port` | Kill the process on a specific port |
| `restart_and_verify` | Restart a project and wait until it's healthy |
| `get_errors` | Get recent stderr/panics/exceptions for a project |
| `find_port_for_project` | Look up which port(s) a project is using |
| `find_available_port` | Find a free TCP port (avoid conflicts) |
| `get_effective_env` | See merged environment variables (.env + secrets) |
| `list_secrets` / `get_secret` / `set_secret` | Manage encrypted secrets |

### Stack orchestration tools

| Tool | What it does |
|------|-------------|
| `list_stacks` | List all defined stacks with running status |
| `get_stack` | Detailed status: per-service health, ports, dependency graph |
| `start_stack` | Start services in dependency order, verify each is healthy |
| `stop_stack` | Graceful shutdown in reverse dependency order |
| `diagnose_stack` | Find the root cause of failures across services |
| `validate_env` | Pre-flight check: port conflicts, missing env vars |

---

## CLI Reference

| Command | Description |
|---------|-------------|
| `havn` | Start the dashboard server at `localhost:9390` |
| `havn status` | Show all projects, ports, frameworks, uptime |
| `havn kill <port\|name>` | Kill process by port number or project name |
| `havn restart <name>` | Kill and restart using the configured start command |
| `havn set-start-cmd <name> "<cmd>"` | Set the start command for a project |
| `havn add <path>` | Manually register a project directory |
| `havn remove <name>` | Unregister a project |
| `havn config <key> [value]` | Get or set configuration (omit value to read) |
| `havn secret set <key> <value>` | Store an encrypted secret (AES-256-GCM) |
| `havn secret get <key>` | Retrieve a decrypted secret |
| `havn secret list` | List secret keys (values hidden) |
| `havn secret delete <key>` | Delete a secret |
| `havn logs` | Tail the server log file |
| `havn mcp` | Start MCP server (stdio transport for AI tools) |
| `havn install-service` | Install as launchd (macOS) or systemd (Linux) service |

### Run as a background service

```bash
havn install-service

# macOS
launchctl load ~/Library/LaunchAgents/com.havn.daemon.plist

# Linux
systemctl --user enable --now havn
```

### Configuration

```bash
havn config dashboard_port 9390    # Dashboard port (default: 9390)
havn config scan_interval 5        # Scan interval in seconds (default: 5)
havn config log_level info         # Log level: trace, debug, info, warn, error
```

Config location:
- **macOS:** `~/Library/Application Support/havn/config.json`
- **Linux:** `~/.config/havn/config.json`

---

## Dashboard

```bash
havn
```

Opens a real-time web dashboard at `http://localhost:9390` showing all running projects, ports, frameworks, and uptime. Kill or restart processes with one click. Live updates via WebSocket.

---

## Framework detection

havn auto-detects frameworks by scanning project files:

| Framework | How it's detected |
|-----------|-------------------|
| Next.js | `package.json` with `next` dependency |
| Vite | `package.json` with `vite` dependency |
| Express | `package.json` with `express` dependency |
| React (CRA) | `package.json` with `react-scripts` |
| Rust (web) | `Cargo.toml` with `axum` / `actix-web` / `rocket` |
| Go | `go.mod` present |
| Django | `manage.py` present |
| FastAPI | `pyproject.toml` with `fastapi` |
| Rails | `Gemfile` with `rails` |
| Docker Compose | `docker-compose.yml` present |

---

## How it works

```
 lsof scan (every 5s)
       |
       v
  +-----------+     +----------+     +-----------+
  | Scanner   | --> | Registry | --> | Dashboard |  (WebSocket)
  | (lsof +   |     | (SQLite) |     | (web UI)  |
  |  kqueue)  |     +----------+     +-----------+
  +-----------+          |
       |                 v
  +-----------+     +----------+
  | Framework | --> | MCP      |  (stdio transport)
  | detector  |     | Server   |
  +-----------+     +----------+
```

1. **Scanner** polls `lsof` every 5s to find listening TCP ports
2. **Project resolver** walks up from each process's working directory to find project roots
3. **Framework detector** parses project files to identify the framework
4. **Registry** (SQLite) persists project history across restarts
5. **Dashboard** streams real-time updates via WebSocket
6. **MCP server** (rmcp) lets AI agents query and control everything

---

## Security

> Designed for **single-user local development machines**.

| Feature | Status |
|---------|--------|
| Secrets at rest | AES-256-GCM encrypted, master key `0600` permissions |
| CORS | Rejects cross-origin requests |
| Binding | `127.0.0.1` only (not exposed to network) |
| Rate limiting | Destructive endpoints (kill, restart) are rate-limited |
| Command validation | Start commands checked against dangerous patterns |

**Limitations:**
- No authentication (safe on single-user machines, not shared systems)
- No HTTPS (acceptable for localhost-only)
- Start commands run as your user (only set commands you trust)
- macOS-optimized (kqueue). Linux falls back to polling.

**Recommendations:**
- Don't bind to `0.0.0.0` in untrusted environments
- Firewall port 9390 on shared machines
- Review start commands before setting them

---

## Contributing

Contributions welcome! Open an issue or pull request.

---

## License

[![BSL 1.1](https://img.shields.io/badge/license-BSL--1.1-orange)](LICENSE)

[Business Source License 1.1](LICENSE) ... free for individuals and non-commercial use. Commercial use by organizations with more than 10 employees requires a license. Automatically converts to **Apache 2.0** on **2030-03-28**.
