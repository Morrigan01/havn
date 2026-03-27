use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::scanner::types::ScanResult;
use crate::ws::WsEvent;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: i64,
    pub path: String,
    pub name: String,
    pub framework: Option<String>,
    pub preferred_port: Option<u16>,
    pub favorite: bool,
    pub start_cmd: Option<String>,
    pub last_seen: String,
    pub ports: Vec<u16>,
    pub pids: Vec<u32>,
    pub uptime_seconds: u64,
}

pub struct Registry {
    conn: Mutex<Connection>,
}

impl Registry {
    pub fn open(db_path: &Path) -> Self {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let conn = match Connection::open(db_path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("SQLite open failed: {}. Creating fresh DB.", e);
                let corrupt = db_path.with_extension("db.corrupt");
                std::fs::rename(db_path, &corrupt).ok();
                Connection::open(db_path).expect("Failed to create fresh DB")
            }
        };

        conn.execute_batch("PRAGMA journal_mode=WAL;").ok();
        Self::migrate(&conn);

        Self {
            conn: Mutex::new(conn),
        }
    }

    fn migrate(conn: &Connection) {
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS projects (
                id             INTEGER PRIMARY KEY,
                path           TEXT NOT NULL UNIQUE,
                name           TEXT NOT NULL,
                framework      TEXT,
                preferred_port INTEGER,
                favorite       INTEGER DEFAULT 0,
                start_cmd      TEXT,
                last_seen      TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS port_history (
                id         INTEGER PRIMARY KEY,
                project_id INTEGER NOT NULL REFERENCES projects(id),
                port       INTEGER NOT NULL,
                pid        INTEGER NOT NULL,
                started_at TEXT NOT NULL,
                stopped_at TEXT,
                UNIQUE(project_id, port, started_at)
            );
            ",
        )
        .expect("Failed to run migrations");
    }

    /// Update registry from a scan cycle. Returns WebSocket events for changes.
    pub fn update_from_scan(&self, results: &[ScanResult]) -> Vec<WsEvent> {
        let conn = self.conn.lock().unwrap();
        let mut events = Vec::new();
        let now = chrono::Utc::now().to_rfc3339();

        let tx = conn.unchecked_transaction().unwrap();

        for result in results {
            let Some(ref project_root) = result.project_root else {
                continue;
            };
            let project_name = result.project_name.as_deref().unwrap_or("unknown");

            // Upsert project
            let existing_id: Option<i64> = tx
                .query_row(
                    "SELECT id FROM projects WHERE path = ?1",
                    [project_root],
                    |row| row.get(0),
                )
                .ok();

            let project_id = if let Some(id) = existing_id {
                tx.execute(
                    "UPDATE projects SET last_seen = ?1, framework = COALESCE(?2, framework), \
                     start_cmd = COALESCE(?3, start_cmd) WHERE id = ?4",
                    rusqlite::params![now, result.framework, result.start_cmd, id],
                )
                .ok();
                id
            } else {
                tx.execute(
                    "INSERT INTO projects (path, name, framework, start_cmd, last_seen) \
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![project_root, project_name, result.framework, result.start_cmd, now],
                )
                .ok();
                let id = tx.last_insert_rowid();

                events.push(WsEvent::ProjectAdded {
                    data: self.get_project_from_tx(&tx, id),
                });
                id
            };

            // Record port activity
            let active: bool = tx
                .query_row(
                    "SELECT COUNT(*) > 0 FROM port_history \
                     WHERE project_id = ?1 AND port = ?2 AND stopped_at IS NULL",
                    rusqlite::params![project_id, result.port],
                    |row| row.get(0),
                )
                .unwrap_or(false);

            if !active {
                tx.execute(
                    "INSERT OR IGNORE INTO port_history (project_id, port, pid, started_at) \
                     VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![project_id, result.port, result.pid, now],
                )
                .ok();

                events.push(WsEvent::PortStarted {
                    project_id,
                    port: result.port,
                });
            }
        }

        // Mark stopped ports: any active port_history entries whose port is not in current results
        let active_ports: Vec<u16> = results.iter().map(|r| r.port).collect();
        if let Ok(mut stmt) = tx.prepare(
            "SELECT id, project_id, port FROM port_history WHERE stopped_at IS NULL",
        ) {
            let rows: Vec<(i64, i64, u16)> = stmt
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
                .unwrap()
                .filter_map(|r| r.ok())
                .collect();

            for (id, project_id, port) in rows {
                if !active_ports.contains(&port) {
                    tx.execute(
                        "UPDATE port_history SET stopped_at = ?1 WHERE id = ?2",
                        rusqlite::params![now, id],
                    )
                    .ok();
                    events.push(WsEvent::PortStopped { project_id, port });
                }
            }
        }

        tx.commit().ok();
        events
    }

    pub fn get_all_projects(&self) -> Vec<Project> {
        let conn = self.conn.lock().unwrap();
        self.get_projects_from_conn(&conn)
    }

    fn get_projects_from_conn(&self, conn: &Connection) -> Vec<Project> {
        let mut stmt = conn
            .prepare(
                "SELECT id, path, name, framework, preferred_port, favorite, start_cmd, last_seen \
                 FROM projects ORDER BY favorite DESC, last_seen DESC",
            )
            .unwrap();

        let projects: Vec<Project> = stmt
            .query_map([], |row| {
                let id: i64 = row.get(0)?;
                let favorite_int: i32 = row.get(5)?;
                Ok(Project {
                    id,
                    path: row.get(1)?,
                    name: row.get(2)?,
                    framework: row.get(3)?,
                    preferred_port: row.get(4)?,
                    favorite: favorite_int != 0,
                    start_cmd: row.get(6)?,
                    last_seen: row.get(7)?,
                    ports: Vec::new(),
                    pids: Vec::new(),
                    uptime_seconds: 0,
                })
            })
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        // Attach active ports to each project
        projects
            .into_iter()
            .map(|mut p| {
                if let Ok(mut port_stmt) = conn.prepare(
                    "SELECT port, pid, started_at FROM port_history \
                     WHERE project_id = ?1 AND stopped_at IS NULL",
                ) {
                    let port_rows: Vec<(u16, u32, String)> = port_stmt
                        .query_map([p.id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
                        .unwrap()
                        .filter_map(|r| r.ok())
                        .collect();

                    for (port, pid, started_at) in &port_rows {
                        p.ports.push(*port);
                        p.pids.push(*pid);

                        if let Ok(started) = chrono::DateTime::parse_from_rfc3339(started_at) {
                            let dur = chrono::Utc::now()
                                .signed_duration_since(started)
                                .num_seconds();
                            if dur > 0 {
                                p.uptime_seconds = p.uptime_seconds.max(dur as u64);
                            }
                        }
                    }
                }
                p
            })
            .collect()
    }

    fn get_project_from_tx(&self, conn: &rusqlite::Transaction, id: i64) -> Project {
        conn.query_row(
            "SELECT id, path, name, framework, preferred_port, favorite, start_cmd, last_seen \
             FROM projects WHERE id = ?1",
            [id],
            |row| {
                let favorite_int: i32 = row.get(5)?;
                Ok(Project {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    name: row.get(2)?,
                    framework: row.get(3)?,
                    preferred_port: row.get(4)?,
                    favorite: favorite_int != 0,
                    start_cmd: row.get(6)?,
                    last_seen: row.get(7)?,
                    ports: Vec::new(),
                    pids: Vec::new(),
                    uptime_seconds: 0,
                })
            },
        )
        .unwrap_or(Project {
            id,
            path: String::new(),
            name: "unknown".to_string(),
            framework: None,
            preferred_port: None,
            favorite: false,
            start_cmd: None,
            last_seen: String::new(),
            ports: Vec::new(),
            pids: Vec::new(),
            uptime_seconds: 0,
        })
    }

    pub fn get_project(&self, id: i64) -> Option<Project> {
        let projects = self.get_all_projects();
        projects.into_iter().find(|p| p.id == id)
    }

    pub fn update_project(&self, id: i64, favorite: Option<bool>, preferred_port: Option<u16>) {
        let conn = self.conn.lock().unwrap();
        if let Some(fav) = favorite {
            conn.execute(
                "UPDATE projects SET favorite = ?1 WHERE id = ?2",
                rusqlite::params![fav as i32, id],
            )
            .ok();
        }
        if let Some(port) = preferred_port {
            conn.execute(
                "UPDATE projects SET preferred_port = ?1 WHERE id = ?2",
                rusqlite::params![port, id],
            )
            .ok();
        }
    }

    pub fn add_project(&self, path: &str, name: &str) -> i64 {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR IGNORE INTO projects (path, name, last_seen) VALUES (?1, ?2, ?3)",
            rusqlite::params![path, name, now],
        )
        .ok();
        conn.last_insert_rowid()
    }

    pub fn remove_project(&self, id: i64) {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM port_history WHERE project_id = ?1", [id])
            .ok();
        conn.execute("DELETE FROM projects WHERE id = ?1", [id]).ok();
    }

    pub fn find_project_by_name(&self, name: &str) -> Option<Project> {
        self.get_all_projects()
            .into_iter()
            .find(|p| p.name == name || p.path == name)
    }
}
