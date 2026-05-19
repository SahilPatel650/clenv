use crate::env::{EnvKind, Environment, HealthStatus};
use std::path::Path;

fn has_broken_symlinks(path: &Path) -> bool {
    walkdir::WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .any(|entry| {
            let p = entry.path();
            p.is_symlink() && !p.exists()
        })
}

fn missing_interpreter(env: &Environment) -> bool {
    match env.kind {
        EnvKind::Python => {
            let python = env.path.join("bin").join("python");
            python.is_symlink() && !python.exists()
        }
        EnvKind::Node => {
            let node = env.path.join("bin").join("node");
            node.is_symlink() && !node.exists()
        }
        _ => false,
    }
}

fn stale_lock_file(env: &Environment) -> bool {
    let (lock_file, marker) = match env.kind {
        EnvKind::Python => (
            env.path.parent().map(|p| p.join("Pipfile.lock")),
            Some(env.path.join("lib")),
        ),
        EnvKind::Node => (
            env.path.parent().map(|p| p.join("package-lock.json")),
            Some(env.path.join(".package-lock.json")),
        ),
        _ => return false,
    };

    let (Some(lock), Some(marker)) = (lock_file, marker) else {
        return false;
    };
    if !lock.exists() || !marker.exists() {
        return false;
    }
    let lock_time = lock.metadata().and_then(|m| m.modified()).ok();
    let marker_time = marker.metadata().and_then(|m| m.modified()).ok();
    match (lock_time, marker_time) {
        (Some(l), Some(m)) => l > m,
        _ => false,
    }
}

/// Compute and set `env.health`. Requires metrics to be computed first.
pub fn compute(env: &mut Environment) {
    let mut warnings: Vec<String> = vec![];
    let mut broken: Vec<String> = vec![];

    if missing_interpreter(env) {
        broken.push("Missing or broken interpreter symlink".to_string());
    }

    if has_broken_symlinks(&env.path) {
        warnings.push("Dangling symlinks detected inside env".to_string());
    }

    if stale_lock_file(env) {
        warnings.push("Lock file is newer than last install".to_string());
    }

    if env.size_bytes > 0 && env.cache_size_bytes > env.size_bytes / 5 {
        warnings.push(format!(
            "Cache is {:.0}% of total env size",
            env.cache_size_bytes as f64 / env.size_bytes as f64 * 100.0
        ));
    }

    if env.version.is_none() && matches!(env.kind, EnvKind::Python | EnvKind::Node) {
        warnings.push("Could not determine interpreter version".to_string());
    }

    if env.package_count == Some(0) {
        warnings.push("No packages installed".to_string());
    }

    env.health = if !broken.is_empty() {
        HealthStatus::Broken(broken)
    } else if !warnings.is_empty() {
        HealthStatus::Warnings(warnings)
    } else {
        HealthStatus::Ok
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::Environment;
    use tempfile::tempdir;

    fn make_env(kind: EnvKind, dir: &std::path::Path) -> Environment {
        let mut env = Environment::new(kind, dir.to_path_buf());
        env.size_bytes = 1000;
        env
    }

    #[test]
    fn healthy_env_gets_ok_status() {
        let dir = tempdir().unwrap();
        let mut env = make_env(EnvKind::Cargo, dir.path());
        env.package_count = Some(5);
        env.version = Some("1.0.0".to_string());
        compute(&mut env);
        assert!(matches!(env.health, HealthStatus::Ok));
    }

    #[test]
    fn empty_env_gets_warning() {
        let dir = tempdir().unwrap();
        let mut env = make_env(EnvKind::Python, dir.path());
        env.package_count = Some(0);
        env.version = Some("Python 3.11.4".to_string());
        compute(&mut env);
        assert!(matches!(env.health, HealthStatus::Warnings(_)));
    }

    #[test]
    fn large_cache_ratio_warns() {
        let dir = tempdir().unwrap();
        let mut env = make_env(EnvKind::Python, dir.path());
        env.size_bytes = 1000;
        env.cache_size_bytes = 300; // 30% > 20% threshold
        env.version = Some("Python 3.11.4".to_string());
        env.package_count = Some(5);
        compute(&mut env);
        assert!(matches!(env.health, HealthStatus::Warnings(_)));
    }

    #[test]
    fn broken_symlink_triggers_warning() {
        let dir = tempdir().unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/nonexistent/target", dir.path().join("dangling"))
                .unwrap();
            let mut env = make_env(EnvKind::Python, dir.path());
            env.version = Some("Python 3.11".to_string());
            env.package_count = Some(3);
            compute(&mut env);
            assert!(matches!(env.health, HealthStatus::Warnings(_)));
        }
        #[cfg(not(unix))]
        {
            // Symlink test only runs on Unix
        }
    }
}
