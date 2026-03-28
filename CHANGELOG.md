# Changelog

All notable changes to havn are documented here.

## [0.3.0] - 2026-03-28 — Full Local Dev Coverage

havn now covers the entire local dev lifecycle with 24 MCP tools. AI agents can discover, control, debug, and orchestrate your full dev environment without you pasting a single terminal command.

### Added

- **Stack orchestration** — 6 new MCP tools (`list_stacks`, `get_stack`, `start_stack`, `stop_stack`, `diagnose_stack`, `validate_env`). Agents can start a full dev stack in dependency order, diagnose cross-service failures, and validate environment before startup.
- **Docker awareness** — `docker_status` tool lists running containers with port mappings and health. Agents can see if Postgres, Redis, or MongoDB are running in Docker.
- **Database connectivity** — `db_status` tool finds database URLs from env vars and .env files, tests TCP connectivity, and detects DB containers in Docker.
- **Dependency freshness** — `check_deps` tool detects stale `node_modules`, outdated cargo builds, and missing Python virtualenvs. Returns fix commands.
- **Resource monitoring** — `get_resources` tool shows CPU and memory usage per project process.
- **Agent helper tools** — `get_logs` (stdout/stderr after restart), `run_command` (run commands in project dir with safety blocking), `health_check` (verify port is responding with latency).
- **Self-update** — `havn update` downloads the latest release binary and replaces itself. Startup version check prints a notice when outdated.
- **Version MCP tool** — `get_version` lets agents check if havn is current.
- **`havn tools` command** — Lists all 24 MCP tools organized by category with MCP config snippet.
- **Shell completions** — `havn completions bash|zsh|fish` generates tab-completion scripts.
- **GitHub Actions release workflow** — Tag-triggered builds for macOS (arm64 + x86_64) and Linux x86_64, uploaded to GitHub Releases.
- **Homebrew formula** — `brew install Morrigan01/tap/havn` (once tap repo is created).
- **Dependency graph** — Profiles (stacks) support dependency edges with topological sort and cycle detection for ordered startup/shutdown.
- **Readiness rules** — Per-project health check configuration (port bind or HTTP 200) with configurable timeouts.
- **SQLite hardening** — WAL mode for concurrent access, foreign key enforcement, orphan row cleanup.
- **Blocking kill** — `kill_pid` now waits for process death instead of fire-and-forget SIGTERM.
- **Shared dependency safety** — `stop_stack` skips killing projects that are still needed by other running stacks.

### Changed

- **README** rewritten as MCP-first. Leads with AI agent demo, organized by use case.
- **`--help`** now explains what havn is and shows quick start examples.
- **License** switched from MIT to BSL 1.1 (free for non-commercial use, converts to Apache 2.0 in 2030).

## [0.2.0] - 2026-03-28 — Dashboard & Process Management

### Added

- **Rate limiting** on destructive endpoints (kill, restart) to prevent abuse.
- **Structured logging** with per-project stdout/stderr capture.
- **Simple Icons** for framework badges in the dashboard.
- **Per-process restart** for multi-port projects.
- **Restart loading overlay** with live poll-until-back-online.
- **Nested project context** showing parent/child names for sub-projects.
- **Sidebar tabs** (Projects/Secrets) with clearer stats.
- **Manual light/dark mode toggle** with localStorage persistence.

### Fixed

- Self-restart via detached shell so the new process can bind the port.
- Restart by port (not stale PID) using live lsof query.
- Filter `/opt/homebrew` paths and auto-clean stale system projects.
- Restore filter input focus and cursor position after render.

## [0.1.0] - 2026-03-27 — Initial Release

### Added

- **Port scanner** — polls `lsof` every 5 seconds to find listening TCP ports.
- **Project resolver** — walks up from process working directory to find project roots.
- **Framework detection** — Next.js, Vite, Express, React, Rust, Go, Django, FastAPI, Rails, Docker Compose.
- **Web dashboard** — real-time at `localhost:9390` via WebSocket.
- **CLI** — `status`, `kill`, `add`, `remove`, `config`, `logs`, `restart`, `set-start-cmd`.
- **MCP server** — 10 tools for AI agents (`list_projects`, `get_project`, `kill_port`, `restart_and_verify`, `get_errors`, `find_port_for_project`, `find_available_port`, `get_system_overview`, `get_effective_env`, secrets management).
- **Encrypted secrets** — AES-256-GCM with global and per-project scoping.
- **SQLite registry** — persists project history across restarts.
- **macOS kqueue watcher** — native process monitoring (Linux falls back to polling).
- **CORS protection** — rejects cross-origin requests.
- **Localhost binding** — `127.0.0.1` only by default.
- **launchd/systemd service installer** — run as a background daemon.
- **v3 dashboard redesign** — left rail + stacked cards + hold-to-kill gesture.
- **Collapsible secrets** with key count badges and global promotion suggestions.
