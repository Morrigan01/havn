use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::scanner::types::ScanResult;
use crate::ws::WsEvent;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: i64,
    pub name: String,
    pub project_ids: Vec<i64>,
    pub created_at: String,
}

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

        // Try migration — if it fails, the DB is corrupt
        if Self::try_migrate(&conn).is_err() {
            tracing::warn!("SQLite migration failed. Recreating DB.");
            drop(conn);
            let corrupt = db_path.with_extension("db.corrupt");
            std::fs::rename(db_path, &corrupt).ok();
            let conn = Connection::open(db_path).expect("Failed to create fresh DB");
            conn.execute_batch("PRAGMA journal_mode=WAL;").ok();
            Self::try_migrate(&conn).expect("Migration failed on fresh DB");
            return Self {
                conn: Mutex::new(conn),
            };
        }

        Self {
            conn: Mutex::new(conn),
        }
    }

    fn try_migrate(conn: &Connection) -> Result<(), rusqlite::Error> {
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

            -- project_id = 0 is reserved for global secrets.
            CREATE TABLE IF NOT EXISTS secrets (
                id         INTEGER PRIMARY KEY,
                project_id INTEGER NOT NULL DEFAULT 0,
                key        TEXT    NOT NULL,
                nonce      BLOB    NOT NULL,
                ciphertext BLOB    NOT NULL,
                UNIQUE(project_id, key)
            );

            CREATE TABLE IF NOT EXISTS profiles (
                id         INTEGER PRIMARY KEY,
                name       TEXT NOT NULL UNIQUE,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS profile_projects (
                profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
                project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                PRIMARY KEY (profile_id, project_id)
            );
            ",
        )?;
        Ok(())
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
                    "UPDATE projects SET name = ?1, last_seen = ?2, \
                     framework = COALESCE(?3, framework), \
                     start_cmd = COALESCE(?4, start_cmd) WHERE id = ?5",
                    rusqlite::params![project_name, now, result.framework, result.start_cmd, id],
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

    pub fn set_start_cmd(&self, id: i64, cmd: &str) {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE projects SET start_cmd = ?1 WHERE id = ?2",
            rusqlite::params![cmd, id],
        )
        .ok();
    }

    // ── Secrets ───────────────────────────────────────────────────────────────

    pub fn set_secret(&self, project_id: i64, key: &str, nonce: &[u8], ciphertext: &[u8]) {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO secrets (project_id, key, nonce, ciphertext) \
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![project_id, key, nonce, ciphertext],
        )
        .ok();
    }

    pub fn get_secret(&self, project_id: i64, key: &str) -> Option<(Vec<u8>, Vec<u8>)> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT nonce, ciphertext FROM secrets WHERE project_id = ?1 AND key = ?2",
            rusqlite::params![project_id, key],
            |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Vec<u8>>(1)?)),
        )
        .ok()
    }

    pub fn list_secret_keys(&self, project_id: i64) -> Vec<String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = match conn
            .prepare("SELECT key FROM secrets WHERE project_id = ?1 ORDER BY key")
        {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        let result: Vec<String> = match stmt.query_map([project_id], |row| row.get(0)) {
            Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
            Err(_) => vec![],
        };
        result
    }

    pub fn delete_secret(&self, project_id: i64, key: &str) -> bool {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM secrets WHERE project_id = ?1 AND key = ?2",
            rusqlite::params![project_id, key],
        )
        .map(|n| n > 0)
        .unwrap_or(false)
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

    #[allow(dead_code)]
    pub fn find_project_by_name(&self, name: &str) -> Option<Project> {
        self.get_all_projects()
            .into_iter()
            .find(|p| p.name == name || p.path == name)
    }

    // ── Profiles ──────────────────────────────────────────────────────────────

    pub fn create_profile(&self, name: &str) -> Result<i64, String> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO profiles (name, created_at) VALUES (?1, ?2)",
            rusqlite::params![name, now],
        ).map_err(|e| e.to_string())?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_profiles(&self) -> Vec<Profile> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = match conn.prepare(
            "SELECT id, name, created_at FROM profiles ORDER BY created_at"
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let profiles: Vec<(i64, String, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        profiles.into_iter().map(|(id, name, created_at)| {
            let project_ids = conn
                .prepare("SELECT project_id FROM profile_projects WHERE profile_id = ?1")
                .ok()
                .and_then(|mut s| {
                    s.query_map([id], |r| r.get::<_, i64>(0)).ok().map(|rows| {
                        rows.filter_map(|r| r.ok()).collect()
                    })
                })
                .unwrap_or_default();
            Profile { id, name, project_ids, created_at }
        }).collect()
    }

    pub fn delete_profile(&self, id: i64) {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM profile_projects WHERE profile_id = ?1", [id]).ok();
        conn.execute("DELETE FROM profiles WHERE id = ?1", [id]).ok();
    }

    pub fn add_project_to_profile(&self, profile_id: i64, project_id: i64) {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO profile_projects (profile_id, project_id) VALUES (?1, ?2)",
            rusqlite::params![profile_id, project_id],
        ).ok();
    }

    pub fn remove_project_from_profile(&self, profile_id: i64, project_id: i64) {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM profile_projects WHERE profile_id = ?1 AND project_id = ?2",
            rusqlite::params![profile_id, project_id],
        ).ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_registry() -> (Registry, TempDir) {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let registry = Registry::open(&db_path);
        (registry, tmp)
    }

    #[test]
    fn test_add_and_get_project() {
        let (registry, _tmp) = test_registry();
        let id = registry.add_project("/Users/test/my-app", "my-app");
        assert!(id > 0);

        let projects = registry.get_all_projects();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "my-app");
        assert_eq!(projects[0].path, "/Users/test/my-app");
    }

    #[test]
    fn test_add_duplicate_path_ignored() {
        let (registry, _tmp) = test_registry();
        registry.add_project("/Users/test/my-app", "my-app");
        registry.add_project("/Users/test/my-app", "my-app-duplicate");

        let projects = registry.get_all_projects();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "my-app");
    }

    #[test]
    fn test_remove_project() {
        let (registry, _tmp) = test_registry();
        let id = registry.add_project("/Users/test/my-app", "my-app");
        registry.remove_project(id);

        let projects = registry.get_all_projects();
        assert!(projects.is_empty());
    }

    #[test]
    fn test_update_favorite() {
        let (registry, _tmp) = test_registry();
        let id = registry.add_project("/Users/test/my-app", "my-app");

        registry.update_project(id, Some(true), None);
        let project = registry.get_project(id).unwrap();
        assert!(project.favorite);

        registry.update_project(id, Some(false), None);
        let project = registry.get_project(id).unwrap();
        assert!(!project.favorite);
    }

    #[test]
    fn test_update_preferred_port() {
        let (registry, _tmp) = test_registry();
        let id = registry.add_project("/Users/test/my-app", "my-app");

        registry.update_project(id, None, Some(3000));
        let project = registry.get_project(id).unwrap();
        assert_eq!(project.preferred_port, Some(3000));
    }

    #[test]
    fn test_get_nonexistent_project() {
        let (registry, _tmp) = test_registry();
        assert!(registry.get_project(999).is_none());
    }

    #[test]
    fn test_empty_registry() {
        let (registry, _tmp) = test_registry();
        let projects = registry.get_all_projects();
        assert!(projects.is_empty());
    }

    #[test]
    fn test_find_project_by_name() {
        let (registry, _tmp) = test_registry();
        registry.add_project("/Users/test/my-app", "my-app");
        registry.add_project("/Users/test/other", "other");

        let found = registry.find_project_by_name("my-app");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "my-app");

        let not_found = registry.find_project_by_name("nope");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_find_project_by_path() {
        let (registry, _tmp) = test_registry();
        registry.add_project("/Users/test/my-app", "my-app");

        let found = registry.find_project_by_name("/Users/test/my-app");
        assert!(found.is_some());
    }

    #[test]
    fn test_update_from_scan() {
        let (registry, _tmp) = test_registry();
        let results = vec![
            crate::scanner::types::ScanResult {
                port: 3000,
                pid: 1234,
                cwd: Some("/Users/test/frontend".to_string()),
                project_root: Some("/Users/test/frontend".to_string()),
                project_name: Some("frontend".to_string()),
                framework: Some("nextjs".to_string()),
                start_cmd: Some("npm run dev".to_string()),
            },
            crate::scanner::types::ScanResult {
                port: 8080,
                pid: 5678,
                cwd: Some("/Users/test/api".to_string()),
                project_root: Some("/Users/test/api".to_string()),
                project_name: Some("api".to_string()),
                framework: Some("express".to_string()),
                start_cmd: Some("node server.js".to_string()),
            },
        ];

        let events = registry.update_from_scan(&results);
        assert!(!events.is_empty());

        let projects = registry.get_all_projects();
        assert_eq!(projects.len(), 2);

        let frontend = projects.iter().find(|p| p.name == "frontend").unwrap();
        assert_eq!(frontend.framework.as_deref(), Some("nextjs"));
        assert!(frontend.ports.contains(&3000));
    }

    #[test]
    fn test_port_stopped_on_missing_scan() {
        let (registry, _tmp) = test_registry();

        // First scan: project running on 3000
        let results1 = vec![crate::scanner::types::ScanResult {
            port: 3000,
            pid: 1234,
            cwd: Some("/Users/test/app".to_string()),
            project_root: Some("/Users/test/app".to_string()),
            project_name: Some("app".to_string()),
            framework: Some("nextjs".to_string()),
            start_cmd: None,
        }];
        registry.update_from_scan(&results1);

        let projects = registry.get_all_projects();
        assert_eq!(projects[0].ports.len(), 1);

        // Second scan: project no longer running
        let events = registry.update_from_scan(&[]);

        // Should have a PortStopped event
        let has_stop = events.iter().any(|e| matches!(e, crate::ws::WsEvent::PortStopped { .. }));
        assert!(has_stop);
    }

    #[test]
    fn test_favorites_sorted_first() {
        let (registry, _tmp) = test_registry();
        let id1 = registry.add_project("/Users/test/alpha", "alpha");
        let id2 = registry.add_project("/Users/test/beta", "beta");

        registry.update_project(id2, Some(true), None);

        let projects = registry.get_all_projects();
        assert_eq!(projects[0].name, "beta"); // favorite first
        assert_eq!(projects[1].name, "alpha");

        // Unfavorite beta, favorite alpha
        registry.update_project(id2, Some(false), None);
        registry.update_project(id1, Some(true), None);

        let projects = registry.get_all_projects();
        assert_eq!(projects[0].name, "alpha");
    }

    #[test]
    fn test_db_corruption_recovery() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");

        // Write garbage to the DB file
        std::fs::write(&db_path, "this is not a sqlite database").unwrap();

        // Should recover by creating a fresh DB
        let registry = Registry::open(&db_path);
        let projects = registry.get_all_projects();
        assert!(projects.is_empty());

        // Corrupt file should be renamed
        assert!(tmp.path().join("test.db.corrupt").exists());
    }
}
