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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ws_event_serialization_scan_completed() {
        let event = WsEvent::ScanCompleted {
            timestamp: "2026-03-27T12:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"scan_completed\""));
        assert!(json.contains("\"timestamp\""));
    }

    #[test]
    fn test_ws_event_serialization_project_removed() {
        let event = WsEvent::ProjectRemoved { id: 42 };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"project_removed\""));
        assert!(json.contains("\"id\":42"));
    }

    #[test]
    fn test_ws_event_serialization_port_started() {
        let event = WsEvent::PortStarted {
            project_id: 1,
            port: 3000,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"port_started\""));
        assert!(json.contains("\"port\":3000"));
    }

    #[test]
    fn test_ws_event_serialization_full_sync_empty() {
        let event = WsEvent::FullSync { data: Vec::new() };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"full_sync\""));
        assert!(json.contains("\"data\":[]"));
    }
}
