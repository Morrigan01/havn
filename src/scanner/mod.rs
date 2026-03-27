pub mod lsof;
pub mod project;
pub mod types;
pub mod watcher;

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast;
use types::ScanResult;
use watcher::ProcessWatcher;

use crate::registry::Registry;
use crate::ws::WsEvent;

/// Run the scanner loop.
///
/// On macOS the loop wakes immediately when a watched process exits (kqueue);
/// on other platforms it falls back to polling every `interval_secs` seconds.
pub async fn run_loop(
    registry: Arc<Registry>,
    tx: broadcast::Sender<WsEvent>,
    interval_secs: u64,
) {
    let watcher = ProcessWatcher::spawn();
    let interval = Duration::from_secs(interval_secs);

    loop {
        let results = match scan_and_update(&registry, &tx).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("Scan cycle failed: {}", e);
                vec![]
            }
        };

        // Hand the live PIDs to the watcher so it can detect exits.
        let pids: Vec<u32> = results.iter().map(|r| r.pid).collect();
        watcher.watch_pids(pids);

        // Wait for a process-exit event or the fallback interval.
        watcher.wait_for_event(interval).await;
    }
}

/// Perform a single scan cycle: detect ports, resolve projects, update registry.
/// Returns the scan results so the caller can register PIDs with the watcher.
async fn scan_and_update(
    registry: &Registry,
    tx: &broadcast::Sender<WsEvent>,
) -> Result<Vec<ScanResult>, String> {
    let results = scan_once().await;

    let changes = registry.update_from_scan(&results);
    if !changes.is_empty() {
        for event in changes {
            let _ = tx.send(event);
        }
    }

    // Always send scan_completed so the dashboard knows we're alive.
    let _ = tx.send(WsEvent::ScanCompleted {
        timestamp: chrono::Utc::now().to_rfc3339(),
    });

    Ok(results)
}

/// One-shot scan: detect listening ports and resolve to projects.
pub async fn scan_once() -> Vec<ScanResult> {
    let port_entries = match lsof::scan_listening_ports().await {
        Ok(entries) => entries,
        Err(e) => {
            tracing::error!("{}", e);
            return Vec::new();
        }
    };

    if port_entries.is_empty() {
        return Vec::new();
    }

    // Batch cwd resolution for all unique PIDs
    let unique_pids: Vec<u32> = port_entries
        .iter()
        .map(|e| e.pid)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let cwd_map = lsof::resolve_cwds(&unique_pids).await;

    // Resolve each port entry to a full scan result, filtering out system noise
    let mut results = Vec::new();
    for entry in &port_entries {
        let cwd = cwd_map.get(&entry.pid).cloned();

        // Filter: skip processes whose cwd is outside user home or in system paths
        if let Some(ref cwd_path) = cwd {
            if is_system_process(cwd_path) {
                continue;
            }
        } else {
            // No cwd resolved — skip unless it's on a common dev port
            if !is_common_dev_port(entry.port) {
                continue;
            }
        }

        let project = cwd
            .as_ref()
            .and_then(|c| project::resolve_project_root(std::path::Path::new(c)));

        // Get start command
        let start_cmd = get_start_cmd(entry.pid).await;

        let project_root = project.as_ref().map(|p| p.root.clone());

        // Deduplicate: skip if we already have this port+project combination
        let dominated = results.iter().any(|r: &ScanResult| {
            r.port == entry.port && r.project_root == project_root
        });
        if dominated {
            continue;
        }

        results.push(ScanResult {
            port: entry.port,
            pid: entry.pid,
            cwd,
            project_root,
            project_name: project.as_ref().map(|p| p.name.clone()),
            framework: project.as_ref().and_then(|p| p.framework.clone()),
            start_cmd,
        });
    }

    results
}

/// Filter out system processes that aren't dev servers.
fn is_system_process(cwd: &str) -> bool {
    let system_prefixes = [
        "/System/",
        "/usr/",
        "/Library/",
        "/sbin/",
        "/private/",
        "/opt/homebrew/Cellar/",
        "/opt/homebrew/opt/",
    ];
    // Processes running from homebrew service dirs (postgres, redis) are kept
    // if their cwd is the homebrew prefix — but filtered if deep in Cellar
    system_prefixes.iter().any(|prefix| cwd.starts_with(prefix))
}

/// Common dev ports that should be shown even without a resolved project.
fn is_common_dev_port(port: u16) -> bool {
    // Exclude well-known service ports even if they fall in dev ranges
    const SERVICE_PORTS: &[u16] = &[5432, 5433, 5672, 6379, 6380, 9200, 9300];
    if SERVICE_PORTS.contains(&port) {
        return false;
    }
    matches!(
        port,
        3000..=3999 | 4000..=4999 | 5000..=5999 | 8000..=8999 | 9000..=9999
    )
}

async fn get_start_cmd(pid: u32) -> Option<String> {
    let output = tokio::process::Command::new("ps")
        .args(["-o", "command=", "-p", &pid.to_string()])
        .output()
        .await
        .ok()?;

    if output.status.success() {
        let cmd = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if cmd.is_empty() {
            None
        } else {
            Some(cmd)
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_system_process() {
        assert!(is_system_process("/System/Library/something"));
        assert!(is_system_process("/usr/sbin/httpd"));
        assert!(is_system_process("/Library/Apple/something"));
        assert!(is_system_process("/opt/homebrew/Cellar/node/21.0/bin/node"));
        assert!(is_system_process("/private/var/something"));

        assert!(!is_system_process("/Users/dev/my-project"));
        assert!(!is_system_process("/home/dev/my-project"));
        assert!(!is_system_process("/opt/homebrew")); // bare homebrew prefix is not filtered
    }

    #[test]
    fn test_is_common_dev_port() {
        assert!(is_common_dev_port(3000));
        assert!(is_common_dev_port(3001));
        assert!(is_common_dev_port(4200));
        assert!(is_common_dev_port(5173));
        assert!(is_common_dev_port(8000));
        assert!(is_common_dev_port(8080));
        assert!(is_common_dev_port(9390));

        assert!(!is_common_dev_port(22));    // SSH
        assert!(!is_common_dev_port(80));    // HTTP
        assert!(!is_common_dev_port(443));   // HTTPS
        assert!(!is_common_dev_port(5432));  // PostgreSQL
        assert!(!is_common_dev_port(6379));  // Redis
        assert!(!is_common_dev_port(27017)); // MongoDB
    }
}
