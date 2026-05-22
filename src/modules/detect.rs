use super::{Module, ModuleStatus};
use super::zshrc;
use std::path::Path;
use std::process::Command;

/// Returns true if any of the module's detect commands succeed (exit code 0).
pub fn is_installed(module: &Module) -> bool {
    if module.detect.commands.is_empty() {
        return false;
    }
    module.detect.commands.iter().any(|cmd| {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if parts.is_empty() { return false; }
        Command::new(parts[0])
            .args(&parts[1..])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

/// Returns true if the module has an install command for the current platform.
pub fn has_install_for_platform(module: &Module) -> bool {
    let Some(install) = &module.install else { return false; };
    #[cfg(target_os = "macos")]
    return install.macos.is_some();
    #[cfg(not(target_os = "macos"))]
    return install.linux.is_some();
}

/// Returns names of `depends_on` modules that are not currently installed.
pub fn missing_deps<'a>(module: &'a Module, all_modules: &[Module]) -> Vec<String> {
    module.depends_on.iter()
        .filter(|dep_name| {
            match all_modules.iter().find(|m| &m.name == *dep_name) {
                Some(dep) => !is_installed(dep),
                None => true,
            }
        })
        .cloned()
        .collect()
}

/// Derive the full ModuleStatus for a module.
pub fn module_status(module: &Module, zshrc_path: &Path) -> ModuleStatus {
    let installed = is_installed(module);
    let has_block = zshrc::has_block(zshrc_path, &module.name);

    match (installed, has_block) {
        (_, true) => ModuleStatus::ManagedActive,
        (true, false) => ModuleStatus::ManagedInactive,
        (false, false) => ModuleStatus::NotInstalled,
    }
}
