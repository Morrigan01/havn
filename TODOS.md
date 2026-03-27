# TODOS

## v2 — Event-driven scanning (kqueue/inotify) ✓
Replace 5-second lsof polling with OS-native process event notifications.
**Done:** `src/scanner/watcher.rs` — kqueue EVFILT_PROC + NOTE_EXIT on macOS.
Scanner wakes immediately when a watched PID exits; falls back to the configured
interval on other platforms. Fallback timer still fires so new processes are caught.

## v2 — Restart functionality ✓
Add `scanprojects restart <project>` with user-defined start commands.
**Done:** `scanprojects set-start-cmd <project> "npm run dev"` stores the command
in the registry. `scanprojects restart <project>` kills current processes and
re-spawns via `sh -c`. Restart button shown in dashboard when start_cmd is set.

## v2 — Local secret management ✓
Agent-friendly local secret store so AI coding tools can access API keys.
**Done:** AES-256-GCM encrypted store in SQLite (secrets table). Master key in
`{config_dir}/master.key` (0600). REST endpoints `/secrets`. CLI subcommands
`scanprojects secret set/get/list/delete`. MCP tools `list_secrets`, `get_secret`,
`set_secret`. Per-project scoping via `--project` flag.

## v2 — DESIGN.md ✓
Create a formal design system document for the dashboard.
**Done:** `DESIGN.md` — IBM Plex type system, phosphor green / off-paper tokens, spacing scale,
motion rules, v3 north star directions from Codex + subagent.

## v3 — Dashboard redesign ✓
Implement the v3 north star directions from DESIGN.md.
**Done:** Left rail (220px) with oversized live counts and sidebar tabs (Projects / Secrets).
Stacked cards replace flat rows. Hold-to-kill gesture (600ms, progress bar). Actions
hidden until hover. Manual light/dark toggle persisted in localStorage.

## v3 — .env auto-detection + secret injection ✓
Projects' .env files surfaced in the dashboard; secrets injected on restart.
**Done:** `src/env_file.rs` reads .env/.env.local/.env.{development,production,test}.
Dashboard Secrets tab shows env keys per project with inline edit (writes back to disk).
`restart_project` loads global + project-scoped store secrets and injects them as env vars.
