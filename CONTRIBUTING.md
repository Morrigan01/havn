# Contributing to havn

Thanks for your interest in contributing. havn is a small, focused project and contributions are welcome.

## Getting started

```bash
# Clone the repo
git clone https://github.com/Morrigan01/havn.git
cd havn

# Build
cargo build

# Run tests
cargo test

# Run locally
cargo run
```

The dashboard opens at `http://localhost:9390`. The MCP server runs via `cargo run -- mcp`.

## Project structure

```
src/
  main.rs          Entry point, command routing
  cli.rs           CLI argument parsing and handlers
  server.rs        Axum HTTP server and routes
  api.rs           All API endpoint handlers (~2400 lines, largest file)
  mcp.rs           MCP tool definitions (proxies to HTTP API)
  registry.rs      SQLite schema, queries, toposort, dependency edges
  scanner/         Port scanning (lsof), project detection, framework detection
  config.rs        Configuration loading/saving
  secrets.rs       AES-256-GCM encrypted secret storage
  logs.rs          In-memory per-project log buffer
  rate_limit.rs    Token-bucket rate limiter
  service.rs       launchd/systemd service installer
  ws.rs            WebSocket event types
  env_file.rs      .env file parser

dashboard/
  index.html       Single-page dashboard
  app.js           Dashboard logic (~1300 lines)
  style.css        Dashboard styles (~1300 lines)
```

## How it works

1. Scanner polls `lsof` every 5 seconds to find listening TCP ports
2. Project resolver walks up from process cwd to find project roots
3. Framework detector parses package.json, Cargo.toml, etc.
4. Registry (SQLite) persists everything
5. HTTP API exposes all data
6. MCP server proxies to HTTP API via reqwest
7. Dashboard connects via WebSocket for real-time updates

## Adding a new MCP tool

1. Add the parameter struct in `src/mcp.rs`:
   ```rust
   #[derive(Serialize, Deserialize, JsonSchema)]
   pub struct MyToolParams {
       pub name: String,
   }
   ```

2. Add the HTTP endpoint in `src/api.rs`

3. Add the route in `src/server.rs`

4. Add the MCP tool in `src/mcp.rs` (inside the `#[tool_router] impl McpServer` block):
   ```rust
   #[tool(name = "my_tool", description = "What it does and when to use it")]
   async fn my_tool(&self, params: Parameters<MyToolParams>) -> String {
       // Proxy to HTTP API
   }
   ```

5. Update `havn tools` output in `src/cli.rs`

6. Add a test if the tool has complex logic

## Adding a new framework

Edit `src/scanner/project.rs`. Add a detection function and add it to `detect_framework()`.

## Code style

- No over-engineering. If it's a one-time operation, inline it.
- Error handling: return structured JSON with `status` and `message` fields.
- MCP tools proxy to HTTP endpoints. Business logic lives in `api.rs`, not `mcp.rs`.
- Tests go in `#[cfg(test)]` modules at the bottom of each file.

## Running the test suite

```bash
cargo test
```

Tests use `tempfile` for isolated SQLite databases. No external services needed.

## Pull requests

- One logical change per PR
- Include tests for new logic
- Update `havn tools` output if adding MCP tools
- Update README if adding user-facing features

## License

By contributing, you agree that your contributions will be licensed under the [BSL 1.1](LICENSE).
