use super::{Module, ModuleStatus};
use super::zshrc;
use std::path::Path;
use std::process::Command;

/// Returns true if any of the module's detect commands succeed (exit code 0).
pub fn is_installed(module: &Module) -> bool {
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

/// Derive the full ModuleStatus for a module.
pub fn module_status(module: &Module, zshrc_path: &Path) -> ModuleStatus {
    let installed = is_installed(module);
    let has_block = zshrc::has_block(zshrc_path, &module.name);
    let unmanaged = zshrc::has_unmanaged_config(zshrc_path, module);

    match (installed, has_block, unmanaged) {
        (_, true, _) => ModuleStatus::ManagedActive,
        (true, false, true) => ModuleStatus::InstalledUnmanaged,
        (true, false, false) => ModuleStatus::ManagedInactive,
        (false, false, _) => ModuleStatus::NotInstalled,
    }
}
