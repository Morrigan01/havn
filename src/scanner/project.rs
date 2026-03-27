use std::path::Path;

use super::types::ProjectInfo;

const MAX_WALK_DEPTH: usize = 20;

/// Resolve a process cwd to a project root + framework.
/// If the resolved root sits inside a parent project (monorepo / nested),
/// the name is returned as "parent/child" for context.
pub fn resolve_project_root(cwd: &Path) -> Option<ProjectInfo> {
    let cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
    let home = dirs::home_dir().unwrap_or_default();

    if cwd == Path::new("/") || cwd == home {
        return None;
    }

    let mut info = find_nearest_root(&cwd, &home)?;
    let inner_path = std::path::PathBuf::from(&info.root);

    // Walk up from the inner root's parent — if there's a containing project,
    // prefix the name so "frontend" becomes "alloovium-deployment/frontend".
    if let Some(parent_dir) = inner_path.parent() {
        if parent_dir != Path::new("/") && parent_dir != home {
            if let Some(parent_info) = find_nearest_root(parent_dir, &home) {
                if parent_info.root != info.root {
                    info.name = format!("{}/{}", parent_info.name, info.name);
                }
            }
        }
    }

    Some(info)
}

fn find_nearest_root(start: &Path, home: &Path) -> Option<ProjectInfo> {
    let mut current = start;
    for _ in 0..MAX_WALK_DEPTH {
        if let Some(framework) = detect_framework(current) {
            return Some(ProjectInfo {
                root: current.to_string_lossy().to_string(),
                name: project_name(current),
                framework: Some(framework),
            });
        }
        if current.join(".git").exists() {
            return Some(ProjectInfo {
                root: current.to_string_lossy().to_string(),
                name: project_name(current),
                framework: None,
            });
        }
        match current.parent() {
            Some(parent) if parent != Path::new("/") && parent != home => {
                current = parent;
            }
            _ => break,
        }
    }
    None
}

/// Detect framework from marker files in a directory. Returns framework name.
fn detect_framework(dir: &Path) -> Option<String> {
    // Check in priority order per the plan
    if let Some(fw) = check_package_json(dir) {
        return Some(fw);
    }
    if let Some(fw) = check_cargo_toml(dir) {
        return Some(fw);
    }
    if dir.join("go.mod").exists() {
        return Some("go".to_string());
    }
    if dir.join("manage.py").exists() {
        return Some("django".to_string());
    }
    if let Some(fw) = check_pyproject_toml(dir) {
        return Some(fw);
    }
    if let Some(fw) = check_gemfile(dir) {
        return Some(fw);
    }
    if dir.join("docker-compose.yml").exists() || dir.join("docker-compose.yaml").exists() {
        return Some("docker-compose".to_string());
    }
    if dir.join("fly.toml").exists() {
        return Some("fly".to_string());
    }
    None
}

fn check_package_json(dir: &Path) -> Option<String> {
    let path = dir.join("package.json");
    let content = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;

    let deps = json.get("dependencies").and_then(|d| d.as_object());
    let dev_deps = json.get("devDependencies").and_then(|d| d.as_object());

    let has_dep = |name: &str| -> bool {
        deps.is_some_and(|d| d.contains_key(name))
            || dev_deps.is_some_and(|d| d.contains_key(name))
    };

    if has_dep("next") {
        Some("nextjs".to_string())
    } else if has_dep("vite") {
        Some("vite".to_string())
    } else if has_dep("react-scripts") {
        Some("create-react-app".to_string())
    } else if has_dep("express") {
        Some("express".to_string())
    } else {
        Some("node".to_string())
    }
}

fn check_cargo_toml(dir: &Path) -> Option<String> {
    let path = dir.join("Cargo.toml");
    let content = std::fs::read_to_string(path).ok()?;

    let web_frameworks = ["axum", "actix-web", "rocket", "warp"];
    for fw in &web_frameworks {
        if content.contains(fw) {
            return Some("rust-web".to_string());
        }
    }
    Some("rust".to_string())
}

fn check_pyproject_toml(dir: &Path) -> Option<String> {
    let path = dir.join("pyproject.toml");
    let content = std::fs::read_to_string(path).ok()?;

    if content.contains("fastapi") {
        Some("fastapi".to_string())
    } else if content.contains("django") {
        Some("django".to_string())
    } else if content.contains("flask") {
        Some("flask".to_string())
    } else {
        None
    }
}

fn check_gemfile(dir: &Path) -> Option<String> {
    let path = dir.join("Gemfile");
    let content = std::fs::read_to_string(path).ok()?;

    if content.contains("rails") {
        Some("rails".to_string())
    } else {
        None
    }
}

/// Derive a display name for a project directory.
/// Priority: package.json name > git remote > directory name
fn project_name(dir: &Path) -> String {
    // Try package.json name
    if let Ok(content) = std::fs::read_to_string(dir.join("package.json")) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(name) = json.get("name").and_then(|n| n.as_str()) {
                if !name.is_empty() {
                    return name.to_string();
                }
            }
        }
    }

    // Fall back to directory name
    dir.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_detect_nextjs_project() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("package.json"),
            r#"{"name":"my-app","dependencies":{"next":"14.0.0","react":"18.0.0"}}"#,
        )
        .unwrap();

        let info = resolve_project_root(tmp.path()).unwrap();
        assert_eq!(info.framework.as_deref(), Some("nextjs"));
        assert_eq!(info.name, "my-app");
    }

    #[test]
    fn test_detect_express_project() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("package.json"),
            r#"{"name":"api","dependencies":{"express":"4.18.0"}}"#,
        )
        .unwrap();

        let info = resolve_project_root(tmp.path()).unwrap();
        assert_eq!(info.framework.as_deref(), Some("express"));
    }

    #[test]
    fn test_detect_rust_web_project() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("Cargo.toml"),
            r#"[dependencies]
axum = "0.8"
tokio = "1"
"#,
        )
        .unwrap();

        let info = resolve_project_root(tmp.path()).unwrap();
        assert_eq!(info.framework.as_deref(), Some("rust-web"));
    }

    #[test]
    fn test_detect_git_root_no_framework() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join(".git")).unwrap();

        let info = resolve_project_root(tmp.path()).unwrap();
        assert!(info.framework.is_none());
    }

    #[test]
    fn test_no_markers_returns_none() {
        let tmp = TempDir::new().unwrap();
        // Empty directory with no markers and no .git
        let result = resolve_project_root(tmp.path());
        assert!(result.is_none());
    }

    #[test]
    fn test_walk_up_to_parent() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("package.json"),
            r#"{"name":"mono","dependencies":{"express":"4.0"}}"#,
        )
        .unwrap();
        let sub = tmp.path().join("packages").join("api");
        fs::create_dir_all(&sub).unwrap();

        // cwd is inside a subdirectory — should walk up to the package.json
        let info = resolve_project_root(&sub).unwrap();
        assert_eq!(info.name, "mono");
    }

    #[test]
    fn test_monorepo_nearest_marker_wins() {
        let tmp = TempDir::new().unwrap();
        // Root has package.json
        fs::write(
            tmp.path().join("package.json"),
            r#"{"name":"mono-root","dependencies":{}}"#,
        )
        .unwrap();
        // Sub-package also has package.json
        let sub = tmp.path().join("packages").join("api");
        fs::create_dir_all(&sub).unwrap();
        fs::write(
            sub.join("package.json"),
            r#"{"name":"api","dependencies":{"express":"4.0"}}"#,
        )
        .unwrap();

        let info = resolve_project_root(&sub).unwrap();
        assert_eq!(info.name, "api");
        assert_eq!(info.framework.as_deref(), Some("express"));
    }

    #[test]
    fn test_home_dir_returns_none() {
        let home = dirs::home_dir().unwrap();
        let result = resolve_project_root(&home);
        assert!(result.is_none());
    }

    #[test]
    fn test_django_project() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("manage.py"), "#!/usr/bin/env python").unwrap();

        let info = resolve_project_root(tmp.path()).unwrap();
        assert_eq!(info.framework.as_deref(), Some("django"));
    }

    #[test]
    fn test_go_project() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("go.mod"), "module example.com/app").unwrap();

        let info = resolve_project_root(tmp.path()).unwrap();
        assert_eq!(info.framework.as_deref(), Some("go"));
    }
}
