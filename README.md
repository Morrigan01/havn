<p align="center">
  <h1 align="center">havn</h1>
  <p align="center"><strong>Your AI agent can write code. Now it can run your dev environment too.</strong></p>
  <p align="center">
    <a href="https://github.com/Morrigan01/havn/actions/workflows/ci.yml"><img src="https://github.com/Morrigan01/havn/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
    <a href="https://github.com/Morrigan01/havn/releases/latest"><img src="https://img.shields.io/github/v/release/Morrigan01/havn?label=release&color=blue" alt="Release"></a>
    <a href="LICENSE"><img src="https://img.shields.io/badge/license-BSL--1.1-orange" alt="License"></a>
    <a href="https://modelcontextprotocol.io/"><img src="https://img.shields.io/badge/MCP-compatible-green" alt="MCP"></a>
    <img src="https://img.shields.io/badge/rust-single%20binary-DEA584" alt="Rust">
  </p>
</p>

> *havn* (Danish: harbor) ... where all your dev servers dock.

You're running 5 dev servers. You can't remember which port is which. Your AI agent can't see any of it. You paste `lsof` output into chat. Again.

**havn fixes this in 30 seconds.** One binary. No config. Your agent gets 24 tools to see, control, and orchestrate everything running on your machine.

```
You:   "Start everything for the payments feature"

Agent: Starting postgres...       :5432  healthy (0.8s)
       Starting api-server...     :8080  healthy (2.1s)
       Starting frontend...       :3000  healthy (1.4s)
       Starting worker...         :9001  healthy (0.6s)

       Stack "payments" is running. 4/4 healthy.
```

```
You:   "Why is the frontend broken?"

Agent: Root cause: api-server crashed (ECONNREFUSED on :5432).
       postgres is healthy. frontend depends on api-server.
       Restarting api-server now... healthy (2.1s). Fixed.
```

---

## Install (30 seconds)

**Option A: Download the binary**

```bash
# macOS (Apple Silicon)
curl -L https://github.com/Morrigan01/havn/releases/latest/download/havn-aarch64-apple-darwin -o havn
chmod +x havn && sudo mv havn /usr/local/bin/

# macOS (Intel)
curl -L https://github.com/Morrigan01/havn/releases/latest/download/havn-x86_64-apple-darwin -o havn
chmod +x havn && sudo mv havn /usr/local/bin/

# Linux
curl -L https://github.com/Morrigan01/havn/releases/latest/download/havn-x86_64-unknown-linux-gnu -o havn
chmod +x havn && sudo mv havn /usr/local/bin/
```

**Option B: Build from source**

```bash
git clone https://github.com/Morrigan01/havn.git && cd havn
cargo install --path .
```

**Verify:**

```bash
havn --version
```

---

## Connect your AI agent (10 seconds)

Add this to your MCP settings (Claude Code, Cursor, Windsurf):

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

Done. Your agent now has 24 tools to manage your dev environment.

> **Where does this go?**
> - Claude Code: `~/.claude/settings.json` under `mcpServers`
> - Cursor: Settings > MCP Servers > Add
> - Windsurf: `.windsurfrules` MCP config

---

## What your agent can do now

### See everything

Your agent starts every session knowing what's running, what's healthy, and what's broken.

| Tool | What it does |
|------|-------------|
| `get_system_overview` | Full snapshot of your dev environment. **Start here.** |
| `list_projects` | All running projects with ports, frameworks, uptime |
| `get_resources` | CPU and memory usage per project |
| `docker_status` | Running Docker containers with port mappings |
| `db_status` | Database connectivity (finds URLs from .env, tests connection) |

### Control everything

No more switching terminals. The agent manages your services directly.

| Tool | What it does |
|------|-------------|
| `restart_and_verify` | Restart a service and confirm it's actually healthy |
| `kill_port` | Kill whatever is running on a port |
| `run_command` | Run `npm install`, `cargo build`, migrations in the right directory |
| `health_check` | Check if a port is responding (HTTP status + latency) |
| `check_deps` | Are node_modules / cargo build / pip venv up to date? |

### Orchestrate stacks

Group services into stacks. Start them in dependency order with one command.

| Tool | What it does |
|------|-------------|
| `start_stack` | Start services in dependency order, verify each is healthy |
| `stop_stack` | Graceful shutdown in reverse order (skips shared deps) |
| `diagnose_stack` | Find the root cause when something breaks |
| `validate_env` | Pre-flight check before starting (port conflicts, missing vars) |

### Manage secrets

Encrypted at rest with AES-256-GCM. Injected into services on restart.

| Tool | What it does |
|------|-------------|
| `set_secret` | Store an encrypted secret |
| `get_secret` | Retrieve a decrypted value |
| `get_effective_env` | See all env vars merged (.env files + secrets) |

> Run `havn tools` to see all 24 tools organized by category.

---

## The dashboard

```bash
havn
```

Opens at **localhost:9390**. Real-time via WebSocket. Shows every running project, framework, port, uptime. Click a project to see git status, health, resources, Docker containers, dependency freshness, database connectivity, and logs. Kill or restart with one click.

The Stacks tab lets you manage service groups with Start Stack, Stop Stack, Diagnose, and Validate buttons.

---

## CLI

```bash
havn                          # Start dashboard at localhost:9390
havn status                   # See what's running
havn kill 3000                # Kill process on port 3000
havn restart my-api           # Restart with configured start command
havn set-start-cmd my-api "npm run dev"
havn secret set DB_URL "postgres://..."
havn secret list              # List keys (values hidden)
havn logs                     # Tail server logs
havn mcp                      # Start MCP server for AI tools
havn tools                    # List all 24 MCP tools
havn update                   # Self-update to latest release
havn completions zsh           # Generate shell completions
havn install-service          # Run as background daemon
```

---

## How it works

havn is a single Rust binary. No runtime dependencies. No Docker. No config files.

1. **Scans** `lsof` every 5 seconds to find listening TCP ports
2. **Resolves** each process to its project directory (walks up to find package.json, Cargo.toml, go.mod, etc.)
3. **Detects** the framework (Next.js, Vite, Express, Django, Rails, FastAPI, Go, Rust, Docker Compose)
4. **Persists** everything in SQLite so history survives restarts
5. **Streams** real-time updates to the dashboard via WebSocket
6. **Exposes** 24 MCP tools so AI agents can query and control everything

```
 lsof (every 5s)
       |
       v
  +-----------+     +----------+     +-----------+
  | Scanner   | --> | Registry | --> | Dashboard |  (WebSocket)
  | + kqueue  |     | (SQLite) |     | localhost |
  +-----------+     +----------+     +-----------+
       |                 |
  +-----------+     +----------+
  | Framework | --> |   MCP    |  (stdio, 24 tools)
  | detector  |     |  Server  |
  +-----------+     +----------+
```

---

## Security

Designed for **single-user local development machines**. Not for production. Not for shared servers.

- **Encrypted secrets** (AES-256-GCM, master key with 0600 permissions)
- **Localhost only** (127.0.0.1, never exposed to network)
- **CORS protected** (rejects cross-origin requests)
- **Rate limited** (destructive endpoints: kill, restart)
- **Command validation** (blocks dangerous shell patterns)
- **No auth** (safe on single-user machines, not shared systems)

---

## Keep it updated

havn checks for updates on startup and tells you when a new version exists.

```bash
havn update     # Self-update to latest release
```

Your AI agent can also check: it calls `get_version` and tells you if you're outdated.

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for project structure and how to add MCP tools.

---

## License

[![BSL 1.1](https://img.shields.io/badge/license-BSL--1.1-orange)](LICENSE)

[Business Source License 1.1](LICENSE) ... free for individuals and non-commercial use. Commercial use by organizations with more than 10 employees requires a license. Converts to **Apache 2.0** on **2030-03-28**.
