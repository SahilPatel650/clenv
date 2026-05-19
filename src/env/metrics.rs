use crate::env::{EnvKind, Environment};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Recursively sum file sizes under `path`.
pub fn dir_size(path: &Path) -> u64 {
    walkdir::WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

/// Most recent modification time of any file under `path`.
pub fn dir_last_modified(path: &Path) -> Option<SystemTime> {
    walkdir::WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .filter_map(|m| m.modified().ok())
        .max()
}

fn cache_paths_for(env: &Environment) -> Vec<PathBuf> {
    match env.kind {
        EnvKind::Python => walkdir::WalkDir::new(&env.path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_dir() && e.file_name() == "__pycache__")
            .map(|e| e.path().to_path_buf())
            .collect(),
        EnvKind::Node => {
            let cache = env.path.parent().map(|p| p.join(".cache"));
            cache.filter(|p| p.exists()).into_iter().collect()
        }
        EnvKind::Conda => {
            let pkgs = env.path.join("pkgs");
            if pkgs.exists() { vec![pkgs] } else { vec![] }
        }
        EnvKind::Cargo => ["debug", "release"]
            .iter()
            .map(|s| env.path.join(s))
            .filter(|p| p.exists())
            .collect(),
        EnvKind::Go => {
            let cache = env.path.join("cache");
            if cache.exists() { vec![cache] } else { vec![] }
        }
        _ => vec![],
    }
}

fn activation_cmd_for(env: &Environment) -> Option<String> {
    match env.kind {
        EnvKind::Python => {
            let activate = env.path.join("bin").join("activate");
            if activate.exists() {
                Some(format!("source {}", activate.display()))
            } else {
                None
            }
        }
        EnvKind::Conda => Some(format!("conda activate {}", env.name)),
        EnvKind::Java => Some(format!("sdk use java {}", env.name)),
        EnvKind::Node => {
            if env.path.to_string_lossy().contains(".nvm") {
                Some(format!("nvm use {}", env.name))
            } else {
                None
            }
        }
        EnvKind::Ruby => Some(format!("rbenv shell {}", env.name)),
        _ => None,
    }
}

fn version_for(env: &Environment) -> Option<String> {
    match env.kind {
        EnvKind::Python => {
            let python = env.path.join("bin").join("python");
            let out = std::process::Command::new(&python)
                .arg("--version")
                .output()
                .ok()?;
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let s2 = String::from_utf8_lossy(&out.stderr).trim().to_string();
            let v = if s.is_empty() { s2 } else { s };
            if v.is_empty() { None } else { Some(v) }
        }
        EnvKind::Node => {
            let node = env.path.join("bin").join("node");
            let out = std::process::Command::new(&node)
                .arg("--version")
                .output()
                .ok()?;
            let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if v.is_empty() { None } else { Some(v) }
        }
        EnvKind::Conda => {
            // Conda env names are arbitrary; run the bundled Python to get the real version.
            for bin in &["python", "python3"] {
                let python = env.path.join("bin").join(bin);
                if let Ok(out) = std::process::Command::new(&python).arg("--version").output() {
                    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    let s2 = String::from_utf8_lossy(&out.stderr).trim().to_string();
                    let v = if s.is_empty() { s2 } else { s };
                    if !v.is_empty() {
                        return Some(v);
                    }
                }
            }
            None
        }
        EnvKind::Ruby | EnvKind::Java => {
            // For rbenv/sdkman the name IS the version (e.g. "3.2.2", "21.0.2-tem").
            if !env.name.is_empty() { Some(env.name.clone()) } else { None }
        }
        EnvKind::Cargo => {
            let cargo_toml = env.path.parent()?.join("Cargo.toml");
            let content = std::fs::read_to_string(cargo_toml).ok()?;
            content
                .lines()
                .find(|l| l.trim_start().starts_with("version"))
                .and_then(|l| l.split('"').nth(1))
                .map(|v| format!("v{v}"))
        }
        EnvKind::Go => {
            let go_mod = env.path.parent()?.join("go.mod");
            let content = std::fs::read_to_string(go_mod).ok()?;
            content
                .lines()
                .find(|l| l.trim_start().starts_with("go "))
                .map(|l| l.trim().to_string())
        }
    }
}

fn package_count_for(env: &Environment) -> Option<usize> {
    match env.kind {
        EnvKind::Python => {
            let pip = env.path.join("bin").join("pip");
            let out = std::process::Command::new(&pip)
                .args(["list", "--format=freeze"])
                .output()
                .ok()?;
            let count = String::from_utf8_lossy(&out.stdout)
                .lines()
                .filter(|l| !l.is_empty())
                .count();
            Some(count)
        }
        EnvKind::Node => std::fs::read_dir(&env.path).ok().map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.file_type().map(|t| t.is_dir()).unwrap_or(false)
                        && !e.file_name().to_string_lossy().starts_with('.')
                })
                .count()
        }),
        EnvKind::Ruby => {
            let gems_dir = env.path.join(".bundle").join("gems");
            std::fs::read_dir(&gems_dir)
                .ok()
                .map(|entries| entries.filter_map(|e| e.ok()).count())
        }
        EnvKind::Conda => {
            let meta_dir = env.path.join("conda-meta");
            std::fs::read_dir(&meta_dir).ok().map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().map(|x| x == "json").unwrap_or(false))
                    .count()
            })
        }
        _ => None,
    }
}

/// Populate all computed fields on an environment.
pub fn compute(env: &mut Environment) {
    env.size_bytes = dir_size(&env.path);
    env.last_accessed = dir_last_modified(&env.path);
    env.cache_paths = cache_paths_for(env);
    env.cache_size_bytes = env.cache_paths.iter().map(|p| dir_size(p)).sum();
    env.version = version_for(env);
    env.package_count = package_count_for(env);
    env.activation_cmd = activation_cmd_for(env);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn dir_size_counts_file_bytes() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "hello").unwrap();
        fs::write(dir.path().join("b.txt"), "world!").unwrap();
        assert_eq!(dir_size(dir.path()), 11);
    }

    #[test]
    fn dir_size_zero_for_empty_dir() {
        let dir = tempdir().unwrap();
        assert_eq!(dir_size(dir.path()), 0);
    }

    #[test]
    fn python_cache_paths_finds_pycache() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("lib").join("__pycache__")).unwrap();
        let env = crate::env::Environment::new(EnvKind::Python, dir.path().to_path_buf());
        let paths = cache_paths_for(&env);
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("__pycache__"));
    }

    #[test]
    fn python_activation_cmd_present_when_activate_exists() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("bin")).unwrap();
        fs::write(dir.path().join("bin").join("activate"), "").unwrap();
        let env = crate::env::Environment::new(EnvKind::Python, dir.path().to_path_buf());
        let cmd = activation_cmd_for(&env);
        assert!(cmd.unwrap().contains("source"));
    }
}
