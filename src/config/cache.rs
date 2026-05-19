use crate::env::Environment;
use anyhow::Result;
use std::path::PathBuf;

pub fn cache_path() -> PathBuf {
    super::config_path()
        .parent()
        .expect("config path always has a parent directory")
        .join("cache.json")
}

pub fn save(envs: &[Environment]) -> Result<()> {
    let path = cache_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string(envs)?;
    std::fs::write(&path, content)?;
    Ok(())
}

pub fn load() -> Result<Vec<Environment>> {
    let path = cache_path();
    if !path.exists() {
        return Ok(vec![]);
    }
    let content = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&content)?)
}
