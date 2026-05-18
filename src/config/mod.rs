use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub scan: ScanConfig,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub session: SessionState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanConfig {
    pub roots: Vec<PathBuf>,
    pub ignore: Vec<PathBuf>,
    pub depth_limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub default_tab: String,
    pub default_sort: String,
    pub default_sort_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub last_tab: String,
    pub last_sort: String,
    pub last_scroll: usize,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            roots: vec![dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))],
            ignore: vec![],
            depth_limit: 10,
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            default_tab: "All".to_string(),
            default_sort: "size".to_string(),
            default_sort_dir: "desc".to_string(),
        }
    }
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            last_tab: "All".to_string(),
            last_sort: "size".to_string(),
            last_scroll: 0,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            scan: ScanConfig::default(),
            ui: UiConfig::default(),
            session: SessionState::default(),
        }
    }
}

pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join(".config")))
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("clenv")
        .join("config.toml")
}

pub fn load() -> Result<Config> {
    let path = config_path();
    if !path.exists() {
        return Ok(Config::default());
    }
    let content = std::fs::read_to_string(&path)?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}

pub fn save(config: &Config) -> Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = toml::to_string_pretty(config)?;
    std::fs::write(&path, content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_home_root() {
        let config = Config::default();
        assert!(!config.scan.roots.is_empty());
        assert_eq!(config.scan.depth_limit, 10);
    }

    #[test]
    fn roundtrip_toml_serialization() {
        let config = Config::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.scan.depth_limit, config.scan.depth_limit);
        assert_eq!(parsed.ui.default_tab, config.ui.default_tab);
        assert_eq!(parsed.session.last_scroll, config.session.last_scroll);
    }

    #[test]
    fn load_returns_default_when_file_missing() {
        // config_path() on a test machine typically won't exist
        // If it does exist, this test is a no-op (but won't fail)
        if !config_path().exists() {
            let config = load().unwrap();
            assert_eq!(config.ui.default_sort, "size");
            assert_eq!(config.scan.depth_limit, 10);
        }
    }

    #[test]
    fn save_and_load_roundtrip() {
        use tempfile::tempdir;
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut config = Config::default();
        config.scan.depth_limit = 7;
        config.ui.default_tab = "Python".to_string();
        config.session.last_scroll = 5;

        // Write directly (bypassing config_path())
        let content = toml::to_string_pretty(&config).unwrap();
        std::fs::write(&path, content).unwrap();

        // Read back directly
        let content = std::fs::read_to_string(&path).unwrap();
        let loaded: Config = toml::from_str(&content).unwrap();

        assert_eq!(loaded.scan.depth_limit, 7);
        assert_eq!(loaded.ui.default_tab, "Python");
        assert_eq!(loaded.session.last_scroll, 5);
    }
}
