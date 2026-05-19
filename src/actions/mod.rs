use crate::env::Environment;
use anyhow::Result;

/// Recursively delete the environment's root directory. Returns bytes freed.
pub fn delete_env(env: &Environment) -> Result<u64> {
    let size = crate::env::metrics::dir_size(&env.path);
    std::fs::remove_dir_all(&env.path)?;
    Ok(size)
}

/// Remove only the cache subdirs from the environment. Returns bytes freed.
pub fn clear_cache(env: &Environment) -> Result<u64> {
    let mut freed: u64 = 0;
    for cache_path in &env.cache_paths {
        if cache_path.exists() {
            freed += crate::env::metrics::dir_size(cache_path);
            std::fs::remove_dir_all(cache_path)?;
        }
    }
    Ok(freed)
}

/// Copy text to the system clipboard.
pub fn copy_to_clipboard(text: &str) -> Result<()> {
    let mut clipboard = arboard::Clipboard::new()?;
    clipboard.set_text(text)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::{EnvKind, Environment};
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn delete_env_removes_directory() {
        let dir = tempdir().unwrap();
        let env_path = dir.path().join("myenv");
        fs::create_dir(&env_path).unwrap();
        fs::write(env_path.join("pyvenv.cfg"), "x").unwrap();
        let env = Environment::new(EnvKind::Python, env_path.clone());
        delete_env(&env).unwrap();
        assert!(!env_path.exists());
    }

    #[test]
    fn clear_cache_removes_only_cache_subdirs() {
        let dir = tempdir().unwrap();
        let env_path = dir.path().join("myenv");
        fs::create_dir_all(env_path.join("lib")).unwrap();
        fs::write(env_path.join("lib").join("important.py"), "code").unwrap();
        let cache = env_path.join("__pycache__");
        fs::create_dir(&cache).unwrap();
        fs::write(cache.join("mod.pyc"), "bytecode").unwrap();

        let mut env = Environment::new(EnvKind::Python, env_path.clone());
        env.cache_paths = vec![cache.clone()];

        let freed = clear_cache(&env).unwrap();
        assert!(freed > 0);
        assert!(!cache.exists());
        assert!(env_path.join("lib").join("important.py").exists());
    }
}
