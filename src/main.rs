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

    let activation_cmd = tui::run(envs, &cfg)?;

    // Save session state
    config::save(&cfg)?;

    // Print activation command to stdout so the shell can eval it
    if let Some(cmd) = activation_cmd {
        println!("{cmd}");
    }

    Ok(())
}
