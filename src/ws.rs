use serde::Serialize;

use crate::registry::Project;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsEvent {
    FullSync { data: Vec<Project> },
    ProjectAdded { data: Project },
    ProjectUpdated { data: Project },
    ProjectRemoved { id: i64 },
    PortStarted { project_id: i64, port: u16 },
    PortStopped { project_id: i64, port: u16 },
    ScanCompleted { timestamp: String },
}
