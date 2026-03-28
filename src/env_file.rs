use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct EnvEntry {
    pub key: String,
    pub value: String,
    /// Just the filename, e.g. ".env.local"
    pub file: String,
    /// Full absolute path — used by the update endpoint
    pub file_path: String,
}

const ENV_FILENAMES: &[&str] = &[
    ".env",
    ".env.local",
    ".env.development",
    ".env.development.local",
    ".env.production",
    ".env.production.local",
    ".env.test",
    ".env.test.local",
];

/// Read all `.env*` files in `project_path` and return their key-value pairs.
pub fn read_env_files(project_path: &str) -> Vec<EnvEntry> {
    let base = Path::new(project_path);
    let mut entries = Vec::new();

    for &filename in ENV_FILENAMES {
        let path = base.join(filename);
        if !path.exists() {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        let file_path = path.to_string_lossy().into_owned();
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = parse_env_line(trimmed) {
                entries.push(EnvEntry {
                    key,
                    value,
                    file: filename.to_string(),
                    file_path: file_path.clone(),
                });
            }
        }
    }

    entries
}

/// Update a single key's value inside `file_path`, writing the change in-place.
/// Returns an error string if the key is not found or the file cannot be written.
pub fn update_env_key(file_path: &str, key: &str, new_value: &str) -> Result<(), String> {
    let content =
        std::fs::read_to_string(file_path).map_err(|e| format!("Cannot read {file_path}: {e}"))?;

    let prefix = format!("{key}=");
    let mut found = false;

    let updated_lines: Vec<String> = content
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with(&prefix) {
                found = true;
                let indent = &line[..line.len() - trimmed.len()];
                format!("{indent}{key}={}", quote_value(new_value))
            } else {
                line.to_string()
            }
        })
        .collect();

    if !found {
        return Err(format!("Key '{key}' not found in {file_path}"));
    }

    let mut new_content = updated_lines.join("\n");
    if content.ends_with('\n') {
        new_content.push('\n');
    }

    std::fs::write(file_path, new_content).map_err(|e| format!("Cannot write {file_path}: {e}"))
}

// ─── helpers ──────────────────────────────────────────────────────────────────

fn parse_env_line(line: &str) -> Option<(String, String)> {
    let (key, rest) = line.split_once('=')?;
    let key = key.trim().to_string();
    if key.is_empty() || key.contains(char::is_whitespace) {
        return None;
    }
    let value = rest.trim();
    // Strip surrounding matching quotes
    let value = strip_quotes(value).to_string();
    Some((key, value))
}

fn strip_quotes(s: &str) -> &str {
    if s.len() >= 2 {
        let (first, last) = (s.as_bytes()[0], s.as_bytes()[s.len() - 1]);
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return &s[1..s.len() - 1];
        }
    }
    s
}

fn quote_value(value: &str) -> String {
    // Only quote if the value contains whitespace, #, or is empty
    if value.is_empty() || value.contains(|c: char| c.is_whitespace() || c == '#') {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        value.to_string()
    }
}
