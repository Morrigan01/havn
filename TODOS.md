# TODOS

## v2 — Event-driven scanning (kqueue/inotify)
Replace 5-second lsof polling with OS-native process event notifications.
**Why:** Eliminates polling overhead and gives instant updates (currently 5s stale).
**How:** Use `kqueue` NOTE_EXEC/NOTE_EXIT on macOS, `inotify`/`netlink` on Linux.
**Blocked by:** v1 ship — this is a performance optimization, not a functional gap.

## v2 — Restart functionality
Add `scanprojects restart <project>` with user-defined start commands.
**Why:** Users want to kill and restart dev servers without switching terminals.
**How:** Store explicit start commands in config (not captured from `ps` — too fragile).
Users configure via `scanprojects config set start_cmd my-api "npm run dev"`.
**Blocked by:** v1 ship — kill-only is reliable; restart needs proper UX design.

## v2 — Local secret management
Agent-friendly local secret store so AI coding tools can access API keys.
**Why:** .env files are insecure and hard for agents to access across projects.
**How:** Encrypted local store, MCP tools for get/set, per-project scoping.
**Blocked by:** v1 ship + security design review.

## v2 — DESIGN.md
Create a formal design system document for the dashboard.
**Why:** As the UI grows (settings page, project detail view), need consistent design tokens.
**How:** Run /design-consultation to produce DESIGN.md with typography, colors, spacing, components.
**Blocked by:** v1 ship — CSS custom properties in the plan are sufficient for v1.
