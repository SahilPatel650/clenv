use crate::env::{EnvSource, Environment};
use anyhow::Result;

/// Human-readable description of the command that will be used to delete this env.
/// Shown in the confirm dialog and printed to the terminal before running.
pub fn delete_preview(env: &Environment) -> String {
    match env.source {
        EnvSource::Filesystem => format!("rm -rf {}", env.path.display()),
        EnvSource::Conda => format!("conda env remove --name {} --yes", env.name),
        EnvSource::Mamba => format!("mamba env remove --name {} --yes", env.name),
        EnvSource::Micromamba => format!("micromamba env remove --name {} --yes", env.name),
        EnvSource::Pyenv => format!("pyenv uninstall --force {}", env.name),
        EnvSource::Rbenv => format!("rbenv uninstall --force {}", env.name),
        EnvSource::Nvm => format!("nvm uninstall {}", env.name),
        EnvSource::Sdkman => format!("sdk uninstall java {}", env.name),
    }
}

/// Returns true when deletion runs an external command whose output should be streamed
/// to the terminal (i.e. the TUI must be suspended first).
pub fn delete_streams_output(env: &Environment) -> bool {
    env.source != EnvSource::Filesystem
}

/// Delete an environment using the appropriate strategy for its source.
///
/// For manager-sourced envs the command runs with inherited stdio — the caller
/// must suspend the TUI (leave alternate screen, disable raw mode) before calling
/// this so the output flows to the visible terminal.
///
/// Returns bytes freed (computed before deletion).
pub fn delete_env(env: &Environment) -> Result<u64> {
    let size = crate::env::metrics::dir_size(&env.path);
    match env.source {
        EnvSource::Filesystem => {
            std::fs::remove_dir_all(&env.path)?;
        }
        EnvSource::Conda => {
            run_manager("conda", &["env", "remove", "--name", &env.name, "--yes"])?;
        }
        EnvSource::Mamba => {
            run_manager("mamba", &["env", "remove", "--name", &env.name, "--yes"])?;
        }
        EnvSource::Micromamba => {
            run_manager("micromamba", &["env", "remove", "--name", &env.name, "--yes"])?;
        }
        EnvSource::Pyenv => {
            run_manager("pyenv", &["uninstall", "--force", &env.name])?;
        }
        EnvSource::Rbenv => {
            run_manager("rbenv", &["uninstall", "--force", &env.name])?;
        }
        EnvSource::Nvm => {
            // nvm is a shell function — must be sourced before use. Names are version
            // strings like "v20.11.0" which cannot contain shell metacharacters.
            run_shell(&format!(
                "source ~/.nvm/nvm.sh 2>/dev/null && nvm uninstall {}",
                &env.name
            ))?;
        }
        EnvSource::Sdkman => {
            // sdk is also a shell function. Identifiers look like "21.0.2-tem".
            run_shell(&format!(
                "source ~/.sdkman/bin/sdkman-init.sh 2>/dev/null && sdk uninstall java {}",
                &env.name
            ))?;
        }
    }
    Ok(size)
}

fn run_manager(bin: &str, args: &[&str]) -> Result<()> {
    let status = std::process::Command::new(bin).args(args).status()?;
    if !status.success() {
        anyhow::bail!("{bin} exited with {status}");
    }
    Ok(())
}

fn run_shell(cmd: &str) -> Result<()> {
    let status = std::process::Command::new("bash").args(["-c", cmd]).status()?;
    if !status.success() {
        anyhow::bail!("shell command failed with {status}");
    }
    Ok(())
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
    fn delete_preview_filesystem() {
        let env = Environment::new(EnvKind::Python, "/tmp/myenv".into());
        assert!(delete_preview(&env).starts_with("rm -rf"));
    }

    #[test]
    fn delete_preview_conda() {
        let mut env = Environment::new(EnvKind::Conda, "/opt/conda/envs/myenv".into());
        env.name = "myenv".to_string();
        env.source = EnvSource::Conda;
        assert_eq!(delete_preview(&env), "conda env remove --name myenv --yes");
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
