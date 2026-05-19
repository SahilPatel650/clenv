use std::fs;
use tempfile::tempdir;

fn make_scan_config(root: std::path::PathBuf) -> clenv::config::ScanConfig {
    clenv::config::ScanConfig {
        roots: vec![root],
        ignore: vec![],
        depth_limit: 5,
    }
}

#[test]
fn scan_detects_python_venv() {
    let dir = tempdir().unwrap();
    let venv = dir.path().join("myenv");
    fs::create_dir(&venv).unwrap();
    fs::write(venv.join("pyvenv.cfg"), "home = /usr/bin").unwrap();

    let envs = clenv::scanner::scan(&make_scan_config(dir.path().to_path_buf()));
    assert!(
        envs.iter().any(|e| matches!(e.kind, clenv::env::EnvKind::Python)),
        "should detect Python venv"
    );
}

#[test]
fn scan_detects_node_modules() {
    let dir = tempdir().unwrap();
    let nm = dir.path().join("node_modules");
    fs::create_dir(&nm).unwrap();
    fs::write(dir.path().join("package.json"), "{}").unwrap();

    let envs = clenv::scanner::scan(&make_scan_config(dir.path().to_path_buf()));
    assert!(
        envs.iter().any(|e| matches!(e.kind, clenv::env::EnvKind::Node)),
        "should detect node_modules"
    );
}

#[test]
fn clear_cache_frees_space() {
    let dir = tempdir().unwrap();
    let env_path = dir.path().join("myenv");
    fs::create_dir_all(env_path.join("lib")).unwrap();
    fs::write(env_path.join("lib").join("site.py"), "important").unwrap();
    let cache = env_path.join("__pycache__");
    fs::create_dir(&cache).unwrap();
    fs::write(cache.join("mod.pyc"), "bytecode data here").unwrap();

    let mut env = clenv::env::Environment::new(
        clenv::env::EnvKind::Python,
        env_path.clone(),
    );
    env.cache_paths = vec![cache.clone()];

    let freed = clenv::actions::clear_cache(&env).unwrap();
    assert!(freed > 0, "should report bytes freed");
    assert!(!cache.exists(), "cache dir should be gone");
    assert!(
        env_path.join("lib").join("site.py").exists(),
        "non-cache files should survive"
    );
}

#[test]
fn delete_env_removes_directory() {
    let dir = tempdir().unwrap();
    let env_path = dir.path().join("myenv");
    fs::create_dir(&env_path).unwrap();
    fs::write(env_path.join("pyvenv.cfg"), "home = /usr/bin").unwrap();

    let env = clenv::env::Environment::new(clenv::env::EnvKind::Python, env_path.clone());
    clenv::actions::delete_env(&env).unwrap();
    assert!(!env_path.exists(), "env directory should be deleted");
}
