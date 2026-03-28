# TODOS — havn

## P1

### kill_pid safety: validate process ownership before killing
- `kill_pid` in `src/api.rs:1298` ignores `expected_path` and kills by PID only
- For automated stack stop/start, must verify PID still belongs to expected project (check cwd via `/proc/{pid}/cwd` or `lsof -p`)
- PID reuse is rare but automated operations make it dangerous
- **Effort:** S | **Depends on:** nothing | **Source:** Codex outside voice (CEO review 2026-03-28)

## P2

### MCP Resource Subscriptions for real-time stack status
- Add MCP resource support: `stack://{name}` with per-project status JSON
- Agents get notified on service crash/start instead of polling
- **Blocker:** `havn mcp` is a separate stdio process, not attached to daemon's WebSocket broadcast. Options: (a) WS client in MCP process connecting to daemon, (b) HTTP long-polling endpoint
- Foundation for auto-healing (restart on crash)
- **Effort:** M (human) / S (CC) | **Depends on:** core stack tools | **Source:** CEO review expansion #4, deferred after Codex outside voice

### Persistent log storage for diagnose_stack
- LogStore in `src/logs.rs` is in-memory only. Daemon restart wipes all stderr history.
- diagnose_stack returns empty `stderr_tail` after restart, making it useless for post-crash debugging.
- Options: (a) persist to SQLite log table with TTL, (b) append-only log file per project
- **Effort:** S (CC: ~15 min) | **Depends on:** core stack tools | **Source:** Codex outside voice (eng review 2026-03-28)

### Co-occurrence-based stack inference
- Track which projects run simultaneously during scanner polls
- After N sessions (5+), suggest stacks based on >80% co-occurrence
- **Blocker:** No session model exists. Must define session boundaries (daemon lifecycle? time-based windows? manual markers?). Without this, long-running background services dominate the co-occurrence matrix.
- Surface via `suggest_stacks` MCP tool (suggestion only, never auto-creates)
- **Effort:** M (human) / S (CC) | **Depends on:** core stack tools + session model | **Source:** CEO review expansion #5, deferred after Codex outside voice

## P3

### Environment Memory — branch-aware snapshots
- Periodically snapshot running state (projects, ports, health) keyed by git branch
- Enable "restore the environment from when it worked" — Codex's insight: "time machine for dev environments"
- Needs: snapshot schema in SQLite, restore logic in start_stack, branch detection
- **Effort:** L (human) / M (CC) | **Depends on:** core stack tools, session model | **Source:** Design doc Approach C, Codex second opinion
