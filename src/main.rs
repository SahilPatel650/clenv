mod actions;
mod config;
mod env;
mod modules;
mod scanner;
mod tui;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "clenv", about = "Index and manage development environments")]
struct Args {
    /// Root directory to scan (overrides config)
    #[arg(short, long)]
    root: Option<PathBuf>,

    /// Additional paths to ignore during scan
    #[arg(long)]
    ignore: Vec<PathBuf>,

    #[command(subcommand)]
    subcommand: Option<TopCommand>,
}

#[derive(Subcommand)]
enum TopCommand {
    /// Manage shell config modules
    Modules(ModulesArgs),
}

#[derive(Parser)]
struct ModulesArgs {
    #[command(subcommand)]
    cmd: Option<ModulesCmd>,

    /// Generate an AI onboarding prompt and print to stdout
    #[arg(long)]
    onboard: bool,
}

#[derive(Subcommand)]
enum ModulesCmd {
    /// List all available modules with their status
    List,
    /// Show details for a specific module
    Show { name: String },
    /// Print enabled-module status summary
    Status,
    /// Enable a module (injects zshrc block)
    Enable { name: String },
    /// Disable a module (removes zshrc block)
    Disable { name: String },
    /// Adopt unmanaged config for a module
    Adopt { name: String },
    /// Verify all enabled modules have correct blocks
    Verify,
    /// Sync the private dotfiles repo
    Sync,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let first_run = config::is_first_run();
    let mut cfg = config::load()?;

    // Dispatch CLI subcommands before launching TUI
    if let Some(TopCommand::Modules(margs)) = args.subcommand {
        handle_modules_cmd(margs, &mut cfg)?;
        return Ok(());
    }

    // CLI --root overrides config; save immediately on first run so wizard sees it
    let show_onboarding = if let Some(root) = args.root {
        cfg.scan.roots = vec![root];
        if first_run {
            config::save(&cfg)?;
        }
        false // user supplied root explicitly — skip wizard
    } else {
        first_run
    };

    if !args.ignore.is_empty() {
        cfg.scan.ignore.extend(args.ignore);
    }

    let cached = config::cache::load().unwrap_or_default();

    let scan_cfg = cfg.scan.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let envs = scanner::scan(&scan_cfg);
        let _ = config::cache::save(&envs);
        let _ = tx.send(envs);
    });

    // Always start the TUI immediately. If we have cached envs show them now;
    // otherwise start with an empty list. Either way, the scan result arrives
    // via scan_rx and the event loop updates the UI when it's ready.
    let (initial_envs, scan_rx) = (cached, Some(rx));

    let (activation_cmd, app) = tui::run(initial_envs, &mut cfg, scan_rx, true, show_onboarding)?;

    // Persist session state on every clean exit
    cfg.session.last_tab = app.active_tab.label().to_string();
    cfg.session.last_sort = match app.sort_field {
        tui::app::SortField::Size => "size",
        tui::app::SortField::Name => "name",
        tui::app::SortField::LastUsed => "last_used",
        tui::app::SortField::Health => "health",
    }
    .to_string();
    cfg.session.last_scroll = app.scroll_offset;
    config::save(&cfg)?;

    if let Some(cmd) = activation_cmd {
        println!("{cmd}");
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// modules subcommand handler
// ---------------------------------------------------------------------------

fn handle_modules_cmd(args: ModulesArgs, cfg: &mut config::Config) -> anyhow::Result<()> {
    let home = dirs::home_dir().unwrap_or_default();
    let zshrc_path = cfg
        .modules
        .zshrc_path
        .clone()
        .unwrap_or_else(|| home.join(".zshrc"));
    let mods = modules::load_builtin_modules();

    if args.onboard {
        print_onboard_prompt(&mods, &zshrc_path, cfg);
        return Ok(());
    }

    match args.cmd {
        None | Some(ModulesCmd::List) => cmd_list(&mods, &zshrc_path, cfg),
        Some(ModulesCmd::Show { name }) => cmd_show(&mods, &name, &zshrc_path),
        Some(ModulesCmd::Status) => cmd_status(&mods, &zshrc_path, cfg),
        Some(ModulesCmd::Enable { name }) => cmd_enable(&mods, &name, &zshrc_path, cfg)?,
        Some(ModulesCmd::Disable { name }) => cmd_disable(&mods, &name, &zshrc_path, cfg)?,
        Some(ModulesCmd::Adopt { name }) => cmd_adopt(&mods, &name, &zshrc_path, cfg)?,
        Some(ModulesCmd::Verify) => cmd_verify(&mods, &zshrc_path, cfg),
        Some(ModulesCmd::Sync) => {
            if let Some(repo_url) = &cfg.modules.private_dotfiles_repo.clone() {
                let private_dir = dirs::home_dir()
                    .unwrap_or_default()
                    .join(".config/clenv/private");
                println!("Syncing {} \u{2192} {:?}", repo_url, private_dir);
                match modules::private_repo::sync(repo_url, &private_dir) {
                    Ok(_) => println!("\u{2713} Done"),
                    Err(e) => { eprintln!("\u{2717} Failed: {e}"); std::process::exit(1); }
                }
            } else {
                println!("No private_dotfiles_repo configured in config.toml");
            }
        }
    }
    Ok(())
}

fn cmd_list(mods: &[modules::Module], zshrc_path: &std::path::Path, cfg: &config::Config) {
    println!("{:<28} {:<20} {}", "NAME", "CATEGORY", "STATUS");
    println!("{}", "-".repeat(60));
    for m in mods {
        let status = modules::detect::module_status(m, zshrc_path);
        let enabled_marker = if cfg.modules.enabled.contains(&m.name) {
            "*"
        } else {
            " "
        };
        println!(
            "{:<28} {:<20} {}{}",
            m.name,
            m.category,
            enabled_marker,
            status.label()
        );
    }
    println!();
    println!("* = enabled in clenv config");
}

fn cmd_show(mods: &[modules::Module], name: &str, zshrc_path: &std::path::Path) {
    let Some(m) = mods.iter().find(|m| m.name == name) else {
        eprintln!("Module not found: {name}");
        return;
    };

    println!("Name:        {}", m.name);
    println!("Description: {}", m.description);
    println!("Category:    {}", m.category);
    println!("Order:       {}", m.zshrc.order);
    if m.zshrc.startup_ms_estimate > 0 {
        println!("Startup est: {}ms", m.zshrc.startup_ms_estimate);
    }

    if !m.detect.commands.is_empty() {
        println!("Detect:");
        for cmd in &m.detect.commands {
            println!("  {cmd}");
        }
    }

    if let Some(install) = &m.install {
        println!("Install:");
        #[cfg(target_os = "macos")]
        if let Some(os) = &install.macos {
            println!("  (macos) {}", os.command);
        }
        #[cfg(not(target_os = "macos"))]
        if let Some(os) = &install.linux {
            println!("  (linux) {}", os.command);
        }
    }

    if !m.depends_on.is_empty() {
        println!("Depends on:  {}", m.depends_on.join(", "));
    }

    // Show up to the first 5 lines of the snippet
    let snippet_preview: String = m
        .zshrc
        .snippet
        .lines()
        .take(5)
        .collect::<Vec<_>>()
        .join("\n");
    let line_count = m.zshrc.snippet.lines().count();
    println!("Snippet ({line_count} lines, first 5):");
    for line in snippet_preview.lines() {
        println!("  {line}");
    }

    let status = modules::detect::module_status(m, zshrc_path);
    println!("Status:      {}", status.label());
}

fn cmd_status(mods: &[modules::Module], zshrc_path: &std::path::Path, cfg: &config::Config) {
    let enabled = &cfg.modules.enabled;
    println!("Enabled modules: {}", enabled.len());
    if enabled.is_empty() {
        println!("  (none)");
        return;
    }
    for name in enabled {
        let block_ok = modules::zshrc::has_block(zshrc_path, name);
        // check that module definition exists
        let exists = mods.iter().any(|m| &m.name == name);
        if block_ok {
            println!("  \u{2713} {name}  — block present");
        } else if exists {
            println!("  \u{2717} {name}  — block missing");
        } else {
            println!("  ? {name}  — unknown module (block {})", if block_ok { "present" } else { "missing" });
        }
    }
}

fn cmd_enable(
    mods: &[modules::Module],
    name: &str,
    zshrc_path: &std::path::Path,
    cfg: &mut config::Config,
) -> anyhow::Result<()> {
    let Some(m) = mods.iter().find(|m| m.name == name) else {
        anyhow::bail!("Module not found: {name}");
    };
    if !m.zshrc.snippet.is_empty() {
        modules::zshrc::write_block(zshrc_path, &m.name, &m.zshrc.snippet)?;
    }
    if !cfg.modules.enabled.contains(&m.name) {
        cfg.modules.enabled.push(m.name.clone());
    }
    config::save(cfg)?;
    println!("\u{2713} enabled {name}");
    Ok(())
}

fn cmd_disable(
    mods: &[modules::Module],
    name: &str,
    zshrc_path: &std::path::Path,
    cfg: &mut config::Config,
) -> anyhow::Result<()> {
    let Some(m) = mods.iter().find(|m| m.name == name) else {
        anyhow::bail!("Module not found: {name}");
    };
    modules::zshrc::remove_block(zshrc_path, &m.name)?;
    cfg.modules.enabled.retain(|n| n != name);
    config::save(cfg)?;
    println!("\u{2713} disabled {name}");
    Ok(())
}

fn cmd_adopt(
    mods: &[modules::Module],
    name: &str,
    zshrc_path: &std::path::Path,
    cfg: &mut config::Config,
) -> anyhow::Result<()> {
    let Some(m) = mods.iter().find(|m| m.name == name) else {
        anyhow::bail!("Module not found: {name}");
    };
    // write_block replaces any existing block or appends — safe to call even if
    // there is unmanaged config because we inject the canonical snippet only
    if !m.zshrc.snippet.is_empty() {
        modules::zshrc::write_block(zshrc_path, &m.name, &m.zshrc.snippet)?;
    }
    if !cfg.modules.enabled.contains(&m.name) {
        cfg.modules.enabled.push(m.name.clone());
    }
    config::save(cfg)?;
    println!("\u{2713} adopted {name}");
    Ok(())
}

fn cmd_verify(mods: &[modules::Module], zshrc_path: &std::path::Path, cfg: &config::Config) {
    let mut all_ok = true;
    for name in &cfg.modules.enabled {
        let block_ok = modules::zshrc::has_block(zshrc_path, name);
        if block_ok {
            println!("\u{2713} {name}");
        } else {
            let defined = mods.iter().any(|m| &m.name == name);
            if defined {
                println!("\u{2717} {name} \u{2014} block missing");
            } else {
                println!("\u{2717} {name} \u{2014} unknown module, block missing");
            }
            all_ok = false;
        }
    }
    if cfg.modules.enabled.is_empty() {
        println!("No modules enabled.");
    } else if all_ok {
        println!("\nAll blocks present.");
    } else {
        println!("\nSome blocks are missing. Run `clenv modules enable <name>` to fix.");
    }
}

fn print_onboard_prompt(
    mods: &[modules::Module],
    zshrc_path: &std::path::Path,
    cfg: &config::Config,
) {
    // Header with embedded agent context doc
    println!("# clenv Module System — Onboarding Prompt");
    println!();
    println!("{}", modules::AGENT_CONTEXT);
    println!();
    println!("---");
    println!();

    // Current module list
    println!("## Current Module List");
    println!();
    cmd_list(mods, zshrc_path, cfg);
    println!();

    // Current .zshrc content (if readable)
    println!("## Current ~/.zshrc Content");
    println!();
    match std::fs::read_to_string(zshrc_path) {
        Ok(contents) => {
            println!("```zsh");
            println!("{contents}");
            println!("```");
        }
        Err(e) => {
            println!("(Could not read {}: {e})", zshrc_path.display());
        }
    }
    println!();

    // Instructions for the AI
    println!("## Instructions");
    println!();
    println!("You are helping manage shell configuration via the clenv module system.");
    println!("Review the current .zshrc above, then:");
    println!();
    println!("1. For each tool already configured in .zshrc but NOT wrapped in clenv markers:");
    println!("   Run: clenv modules adopt <name>");
    println!();
    println!("2. For new tools the user wants:");
    println!("   Run: clenv modules enable <name>");
    println!();
    println!("3. To remove a tool:");
    println!("   Run: clenv modules disable <name>");
    println!();
    println!("4. After all changes, verify:");
    println!("   Run: clenv modules verify");
    println!();
    println!("Do NOT directly edit ~/.zshrc. All changes must go through `clenv modules` CLI commands.");
}
