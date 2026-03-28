use std::collections::HashMap;

use super::types::PortEntry;

/// Parse `lsof -iTCP -sTCP:LISTEN -P -n -F pn` output into port entries.
/// Format: lines starting with 'p' = PID, lines starting with 'n' = name (host:port)
pub fn parse_lsof_ports(output: &str) -> Vec<PortEntry> {
    let mut entries = Vec::new();
    let mut current_pid: Option<u32> = None;

    for line in output.lines() {
        if let Some(pid_str) = line.strip_prefix('p') {
            current_pid = pid_str.trim().parse().ok();
        } else if let Some(name) = line.strip_prefix('n') {
            if let Some(pid) = current_pid {
                if let Some(port) = parse_port_from_name(name.trim()) {
                    // Deduplicate IPv4/IPv6 — same port+pid counts once
                    if !entries
                        .iter()
                        .any(|e: &PortEntry| e.port == port && e.pid == pid)
                    {
                        entries.push(PortEntry { port, pid });
                    }
                }
            }
        }
    }

    entries
}

/// Extract port number from lsof name field like "127.0.0.1:3000" or "[::1]:3000" or "*:3000"
fn parse_port_from_name(name: &str) -> Option<u16> {
    let port_str = name.rsplit(':').next()?;
    port_str.parse().ok()
}

/// Parse `lsof -a -d cwd -Fn -p <pids>` output into pid→cwd map.
/// Format: lines starting with 'p' = PID, lines starting with 'n' = path (cwd)
pub fn parse_lsof_cwd(output: &str) -> HashMap<u32, String> {
    let mut map = HashMap::new();
    let mut current_pid: Option<u32> = None;

    for line in output.lines() {
        if let Some(pid_str) = line.strip_prefix('p') {
            current_pid = pid_str.trim().parse().ok();
        } else if let Some(path) = line.strip_prefix('n') {
            if let Some(pid) = current_pid {
                map.insert(pid, path.trim().to_string());
            }
        }
    }

    map
}

/// Run lsof to get listening TCP ports. Returns raw parsed entries.
pub async fn scan_listening_ports() -> Result<Vec<PortEntry>, String> {
    let output = tokio::process::Command::new("lsof")
        .args(["-iTCP", "-sTCP:LISTEN", "-P", "-n", "-F", "pn"])
        .output()
        .await
        .map_err(|e| format!("lsof not found: {}. Install developer tools.", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // lsof returns exit code 1 when no results found — that's OK
        if output.status.code() == Some(1) && stderr.is_empty() {
            return Ok(Vec::new());
        }
        tracing::warn!("lsof stderr: {}", stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_lsof_ports(&stdout))
}

/// Run lsof to resolve cwd for a set of PIDs. Returns pid→cwd map.
pub async fn resolve_cwds(pids: &[u32]) -> HashMap<u32, String> {
    if pids.is_empty() {
        return HashMap::new();
    }

    let pid_list: String = pids
        .iter()
        .map(|p| p.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let output = tokio::process::Command::new("lsof")
        .args(["-a", "-d", "cwd", "-Fn", "-p", &pid_list])
        .output()
        .await;

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            parse_lsof_cwd(&stdout)
        }
        Err(e) => {
            tracing::warn!("Failed to resolve cwds: {}", e);
            HashMap::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_lsof_ports_happy_path() {
        let output = "\
p1234
fcwd
n*:3000
p5678
fcwd
n127.0.0.1:8080
n[::1]:8080
";
        let entries = parse_lsof_ports(output);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].port, 3000);
        assert_eq!(entries[0].pid, 1234);
        assert_eq!(entries[1].port, 8080);
        assert_eq!(entries[1].pid, 5678);
        // IPv6 duplicate should be deduped
    }

    #[test]
    fn test_parse_lsof_ports_empty() {
        let entries = parse_lsof_ports("");
        assert!(entries.is_empty());
    }

    #[test]
    fn test_parse_lsof_ports_malformed() {
        let output = "garbage\nmore garbage\np\nnot_a_port";
        let entries = parse_lsof_ports(output);
        assert!(entries.is_empty());
    }

    #[test]
    fn test_parse_lsof_cwd_happy_path() {
        let output = "\
p1234
ncwd
n/Users/dev/my-project
p5678
ncwd
n/Users/dev/other-project
";
        let map = parse_lsof_cwd(output);
        // The last 'n' line for each PID wins
        assert!(map.contains_key(&1234));
        assert!(map.contains_key(&5678));
    }

    #[test]
    fn test_parse_port_from_various_formats() {
        assert_eq!(parse_port_from_name("127.0.0.1:3000"), Some(3000));
        assert_eq!(parse_port_from_name("[::1]:8080"), Some(8080));
        assert_eq!(parse_port_from_name("*:5173"), Some(5173));
        assert_eq!(parse_port_from_name("localhost:9000"), Some(9000));
        assert_eq!(parse_port_from_name("garbage"), None);
    }
}
