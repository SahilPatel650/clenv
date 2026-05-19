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

    let mut cfg = config::load()?;

    if let Some(root) = args.root {
        cfg.scan.roots = vec![root];
    }
    if !args.ignore.is_empty() {
        cfg.scan.ignore.extend(args.ignore);
    }

    eprintln!("Scanning {} root(s)…", cfg.scan.roots.len());
    let envs = scanner::scan(&cfg.scan);
    eprintln!("Found {} environments.", envs.len());

    let (activation_cmd, app) = tui::run(envs, &cfg)?;

    // Persist session state
    cfg.session.last_tab = app.active_tab.label().to_string();
    cfg.session.last_sort = match app.sort_field {
        tui::app::SortField::Size => "size",
        tui::app::SortField::Name => "name",
        tui::app::SortField::LastUsed => "last_used",
        tui::app::SortField::Health => "health",
    }.to_string();
    cfg.session.last_scroll = app.scroll_offset;
    config::save(&cfg)?;

    if let Some(cmd) = activation_cmd {
        println!("{cmd}");
    }

    Ok(())
}
