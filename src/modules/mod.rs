pub mod detect;
pub mod installer;
pub mod private_repo;
pub mod resolver;
pub mod zshrc;

use serde::Deserialize;

/// Agent context document embedded at compile time.
pub const AGENT_CONTEXT: &str = include_str!("AGENT_CONTEXT.md");

#[derive(Debug, Clone, Deserialize)]
pub struct Module {
    pub name: String,
    pub description: String,
    pub category: String,
    pub detect: DetectSpec,
    pub install: Option<InstallSpec>,
    pub zshrc: ZshrcSpec,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DetectSpec {
    #[serde(default)]
    pub commands: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InstallSpec {
    pub linux: Option<InstallOs>,
    pub macos: Option<InstallOs>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InstallOs {
    pub command: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ZshrcSpec {
    pub snippet: String,
    #[serde(default)]
    pub startup_ms_estimate: u64,
    #[serde(default = "default_order")]
    pub order: i32,
    pub user_extend: Option<String>,
}

fn default_order() -> i32 { 100 }

#[derive(Debug, Clone, PartialEq)]
pub enum ModuleStatus {
    NotInstalled,
    InstalledUnmanaged,
    ManagedActive,
    ManagedInactive,
}

impl ModuleStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::NotInstalled => "not installed",
            Self::InstalledUnmanaged => "unmanaged",
            Self::ManagedActive => "active",
            Self::ManagedInactive => "inactive",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModuleEntry {
    pub definition: Module,
    pub status: ModuleStatus,
    pub enabled: bool,
}

/// Load all built-in modules. Returns them sorted by zshrc.order.
/// Built-in TOMLs are embedded at compile time via include_str!().
pub fn load_builtin_modules() -> Vec<Module> {
    let raw: &[(&str, &str)] = &[
        ("homebrew-linux", include_str!("builtin/homebrew-linux.toml")),
        ("oh-my-zsh", include_str!("builtin/oh-my-zsh.toml")),
        ("powerlevel10k", include_str!("builtin/powerlevel10k.toml")),
        ("zsh-autosuggestions", include_str!("builtin/zsh-autosuggestions.toml")),
        ("fast-syntax-highlighting", include_str!("builtin/fast-syntax-highlighting.toml")),
        ("fzf", include_str!("builtin/fzf.toml")),
        ("mamba", include_str!("builtin/mamba.toml")),
        ("conda-aliases", include_str!("builtin/conda-aliases.toml")),
        ("uv", include_str!("builtin/uv.toml")),
        ("nvm", include_str!("builtin/nvm.toml")),
        ("bun", include_str!("builtin/bun.toml")),
        ("sdkman", include_str!("builtin/sdkman.toml")),
        ("zoxide", include_str!("builtin/zoxide.toml")),
        ("thefuck", include_str!("builtin/thefuck.toml")),
        ("jabba", include_str!("builtin/jabba.toml")),
    ];

    let mut modules: Vec<Module> = raw
        .iter()
        .filter_map(|(name, content)| {
            toml::from_str(content)
                .map_err(|e| eprintln!("Failed to parse module {name}: {e}"))
                .ok()
        })
        .collect();

    modules.sort_by_key(|m| m.zshrc.order);
    modules
}
