use crate::env::EnvKind;
use std::path::Path;

/// Returns the EnvKind if `path` is a recognized environment root, else None.
pub fn detect_kind(path: &Path) -> Option<EnvKind> {
    if path.join("pyvenv.cfg").exists() {
        return Some(EnvKind::Python);
    }
    if path.is_dir() && path.file_name().map(|n| n == "node_modules").unwrap_or(false) {
        if path.parent().map(|p| p.join("package.json").exists()).unwrap_or(false) {
            return Some(EnvKind::Node);
        }
    }
    if path.join("conda-meta").is_dir() {
        return Some(EnvKind::Conda);
    }
    if path.join(".bundle").join("gems").is_dir() {
        return Some(EnvKind::Ruby);
    }
    if path.file_name().map(|n| n == "target").unwrap_or(false)
        && path.join("CACHEDIR.TAG").exists()
    {
        return Some(EnvKind::Cargo);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn detects_python_venv() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("pyvenv.cfg"), "home = /usr/bin").unwrap();
        assert_eq!(detect_kind(dir.path()), Some(EnvKind::Python));
    }

    #[test]
    fn detects_node_modules() {
        let dir = tempdir().unwrap();
        let nm = dir.path().join("node_modules");
        fs::create_dir(&nm).unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        assert_eq!(detect_kind(&nm), Some(EnvKind::Node));
    }

    #[test]
    fn detects_conda_env() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join("conda-meta")).unwrap();
        assert_eq!(detect_kind(dir.path()), Some(EnvKind::Conda));
    }

    #[test]
    fn detects_ruby_bundle() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".bundle").join("gems")).unwrap();
        assert_eq!(detect_kind(dir.path()), Some(EnvKind::Ruby));
    }

    #[test]
    fn detects_cargo_target() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("target");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("CACHEDIR.TAG"), "Signature: 8a477f597d28d172789f06886806bc55").unwrap();
        assert_eq!(detect_kind(&target), Some(EnvKind::Cargo));
    }

    #[test]
    fn returns_none_for_unrecognized_dir() {
        let dir = tempdir().unwrap();
        assert_eq!(detect_kind(dir.path()), None);
    }
}
