pub mod health;
pub mod metrics;

use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EnvKind {
    Python,
    Node,
    Conda,
    Ruby,
    Cargo,
    Go,
    Java,
}

impl EnvKind {
    pub fn label(&self) -> &'static str {
        match self {
            EnvKind::Python => "Python",
            EnvKind::Node => "Node",
            EnvKind::Conda => "Conda",
            EnvKind::Ruby => "Ruby",
            EnvKind::Cargo => "Cargo",
            EnvKind::Go => "Go",
            EnvKind::Java => "Java",
        }
    }
}

#[derive(Debug, Clone)]
pub enum HealthStatus {
    Unknown,
    Ok,
    Warnings(Vec<String>),
    Broken(Vec<String>),
}

impl HealthStatus {
    pub fn symbol(&self) -> &'static str {
        match self {
            HealthStatus::Unknown => "?",
            HealthStatus::Ok => "✓",
            HealthStatus::Warnings(_) => "⚠",
            HealthStatus::Broken(_) => "✗",
        }
    }

    pub fn messages(&self) -> &[String] {
        match self {
            HealthStatus::Warnings(msgs) | HealthStatus::Broken(msgs) => msgs,
            _ => &[],
        }
    }
}

#[derive(Debug, Clone)]
pub struct Environment {
    pub kind: EnvKind,
    pub path: PathBuf,
    pub name: String,
    pub size_bytes: u64,
    pub cache_size_bytes: u64,
    pub last_accessed: Option<SystemTime>,
    pub version: Option<String>,
    pub package_count: Option<usize>,
    pub health: HealthStatus,
    pub activation_cmd: Option<String>,
    pub cache_paths: Vec<PathBuf>,
}

impl Environment {
    pub fn new(kind: EnvKind, path: PathBuf) -> Self {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());
        Self {
            kind,
            path,
            name,
            size_bytes: 0,
            cache_size_bytes: 0,
            last_accessed: None,
            version: None,
            package_count: None,
            health: HealthStatus::Unknown,
            activation_cmd: None,
            cache_paths: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_kind_labels_are_correct() {
        assert_eq!(EnvKind::Python.label(), "Python");
        assert_eq!(EnvKind::Node.label(), "Node");
        assert_eq!(EnvKind::Java.label(), "Java");
    }

    #[test]
    fn health_symbols() {
        assert_eq!(HealthStatus::Ok.symbol(), "✓");
        assert_eq!(HealthStatus::Warnings(vec![]).symbol(), "⚠");
        assert_eq!(HealthStatus::Broken(vec![]).symbol(), "✗");
    }

    #[test]
    fn new_env_derives_name_from_path() {
        let env = Environment::new(EnvKind::Python, PathBuf::from("/home/user/proj/.venv"));
        assert_eq!(env.name, ".venv");
        assert_eq!(env.size_bytes, 0);
    }
}
