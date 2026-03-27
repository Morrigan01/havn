use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortEntry {
    pub port: u16,
    pub pid: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub port: u16,
    pub pid: u32,
    pub cwd: Option<String>,
    pub project_root: Option<String>,
    pub project_name: Option<String>,
    pub framework: Option<String>,
    pub start_cmd: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProjectInfo {
    pub root: String,
    pub name: String,
    pub framework: Option<String>,
}
