mod actions;
mod config;
mod env;
mod scanner;
mod tui;

use clap::Parser;
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
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let first_run = config::is_first_run();
    let mut cfg = config::load()?;

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

