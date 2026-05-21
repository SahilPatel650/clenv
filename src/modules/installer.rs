use super::Module;

/// Returns the install command that would run for the current OS.
pub fn install_preview(module: &Module) -> Option<String> {
    let install = module.install.as_ref()?;
    #[cfg(target_os = "macos")]
    let os_spec = install.macos.as_ref();
    #[cfg(not(target_os = "macos"))]
    let os_spec = install.linux.as_ref();
    os_spec.map(|s| s.command.clone())
}
