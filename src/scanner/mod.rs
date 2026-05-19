pub mod fs;
pub mod managers;

use crate::config::ScanConfig;
use crate::env::{health, metrics, EnvKind, Environment};
use rayon::prelude::*;
use std::collections::HashSet;
use std::path::PathBuf;
use walkdir::WalkDir;

/// Run a full scan: fs walk + manager discovery, with metrics and health computed.
pub fn scan(config: &ScanConfig) -> Vec<Environment> {
    // Phase 1: collect (path, kind) from parallel fs walk across all roots
    let mut detected: Vec<(PathBuf, EnvKind)> = config
        .roots
        .par_iter()
        .flat_map(|root| walk_root(root, &config.ignore, config.depth_limit))
        .collect();

    // Add Go module cache if it exists
    if let Some(go_path) = go_module_cache() {
        if go_path.exists() {
            detected.push((go_path, EnvKind::Go));
        }
    }

    // Phase 2: build and enrich environments in parallel
    let mut fs_envs: Vec<Environment> = detected
        .into_par_iter()
        .map(|(path, kind)| {
            let mut env = Environment::new(kind, path);
            metrics::compute(&mut env);
            health::compute(&mut env);
            env
        })
        .collect();

    // Phase 3: manager-aware discovery
    let manager_envs = managers::discover_all();

    // Phase 4: dedup — only add manager envs whose canonical path isn't already present,
    // and which fall under one of the configured roots (respecting the ignore list).
    let fs_paths: HashSet<PathBuf> = fs_envs
        .iter()
        .filter_map(|e| e.path.canonicalize().ok())
        .collect();

    let canonical_roots: Vec<PathBuf> = config
        .roots
        .iter()
        .filter_map(|r| r.canonicalize().ok())
        .collect();

    let canonical_ignores: Vec<PathBuf> = config
        .ignore
        .iter()
        .filter_map(|ig| ig.canonicalize().ok())
        .collect();

    for mut env in manager_envs {
        let canonical = env.path.canonicalize().ok();
        let already_present = canonical
            .as_ref()
            .map(|p| fs_paths.contains(p))
            .unwrap_or(false);
        if already_present || !env.path.exists() {
            continue;
        }
        // Only include if the env falls under one of the scan roots
        // and is not under an ignored path.
        let under_root = canonical.as_ref().map(|p| {
            canonical_roots.iter().any(|r| p.starts_with(r))
        }).unwrap_or(false);
        let is_ignored = canonical.as_ref().map(|p| {
            canonical_ignores.iter().any(|ig| p.starts_with(ig))
        }).unwrap_or(false);
        if under_root && !is_ignored {
            metrics::compute(&mut env);
            health::compute(&mut env);
            fs_envs.push(env);
        }
    }

    fs_envs
}

fn go_module_cache() -> Option<PathBuf> {
    let gopath = std::env::var("GOPATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| dirs::home_dir().map(|h| h.join("go")).unwrap_or_else(|| PathBuf::from("/nonexistent")));
    Some(gopath.join("pkg").join("mod"))
}

fn walk_root(root: &std::path::Path, ignore: &[PathBuf], depth_limit: usize) -> Vec<(PathBuf, EnvKind)> {
    let mut results: Vec<(PathBuf, EnvKind)> = vec![];

    let walker = WalkDir::new(root)
        .max_depth(depth_limit)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !ignore.iter().any(|ig| e.path().starts_with(ig)));

    for entry in walker.filter_map(|e| e.ok()) {
        if !entry.file_type().is_dir() {
            continue;
        }
        if let Some(kind) = fs::detect_kind(entry.path()) {
            results.push((entry.path().to_path_buf(), kind));
        }
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ScanConfig;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn scan_finds_python_venv() {
        let dir = tempdir().unwrap();
        let venv = dir.path().join("myenv");
        fs::create_dir(&venv).unwrap();
        fs::write(venv.join("pyvenv.cfg"), "home = /usr/bin").unwrap();

        let config = ScanConfig {
            roots: vec![dir.path().to_path_buf()],
            ignore: vec![],
            depth_limit: 5,
        };
        let envs = scan(&config);
        assert!(envs.iter().any(|e| e.kind == EnvKind::Python));
    }

    #[test]
    fn scan_respects_ignore_list() {
        let dir = tempdir().unwrap();
        let ignored = dir.path().join("ignored");
        fs::create_dir(&ignored).unwrap();
        let venv = ignored.join("myenv");
        fs::create_dir(&venv).unwrap();
        fs::write(venv.join("pyvenv.cfg"), "home = /usr/bin").unwrap();

        let config = ScanConfig {
            roots: vec![dir.path().to_path_buf()],
            ignore: vec![ignored.clone()],
            depth_limit: 5,
        };
        let envs = scan(&config);
        assert!(envs.is_empty());
    }
}
