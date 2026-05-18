# clenv — Environment Index & Manager TUI

**Date:** 2026-05-18  
**Status:** Approved  
**Stack:** Rust, ratatui, walkdir, rayon

---

## Overview

`clenv` is a single-binary terminal UI tool that discovers, metrics, and manages development environments across a developer's machine. It scans a configurable root directory (defaulting to `~`) and also queries installed environment managers directly. It runs on macOS and Ubuntu.

---

## Environment Types Supported

### Filesystem-detected (by heuristic)

| Type | Detection Signal |
|------|-----------------|
| Python venv | `pyvenv.cfg` present inside the directory |
| node_modules | `node_modules/` dir with sibling `package.json` |
| Conda env | `conda-meta/` directory present |
| Ruby bundle | `.bundle/gems/` directory present |
| Cargo build | `target/` dir containing `CACHEDIR.TAG` |
| Go module cache | `$GOPATH/pkg/mod` |

### Manager-aware discovery (queried directly)

- **conda** — `conda env list`
- **nvm** — `nvm list`
- **pyenv** — `pyenv versions`
- **rbenv** — `rbenv versions`
- **sdkman** — `sdk list installed`

Results from manager-aware discovery are deduplicated against filesystem scan results by canonical path.

---

## Data Model

```rust
struct Environment {
    kind: EnvKind,                  // Python, Node, Conda, Ruby, Cargo, Go, Java
    path: PathBuf,
    name: String,                   // inferred from dir name or manager metadata
    size_bytes: u64,
    last_accessed: SystemTime,
    version: Option<String>,        // e.g. "Python 3.11.4", "Node 20.11.0"
    package_count: Option<usize>,   // from pip list, npm ls, gem list, etc.
    health: HealthStatus,           // Ok | Warnings(Vec<String>) | Broken
    activation_cmd: Option<String>, // e.g. "source /path/.venv/bin/activate"
    cache_paths: Vec<PathBuf>,      // subdirs safe to clear without breaking env
}

enum HealthStatus {
    Ok,
    Warnings(Vec<String>),
    Broken(Vec<String>),
}
```

---

## Architecture

```
clenv/
├── src/
│   ├── main.rs           — CLI args (clap), load config, start TUI
│   ├── config/           — config.toml load/save + session state persistence
│   ├── scanner/
│   │   ├── mod.rs        — scan orchestrator, rayon parallel walk, result channel
│   │   ├── fs.rs         — walkdir-based env detection
│   │   └── managers/     — one file per manager: conda, nvm, pyenv, rbenv, sdkman
│   ├── env/
│   │   ├── mod.rs        — Environment struct and EnvKind enum
│   │   ├── metrics.rs    — disk size, package count, version string, last accessed
│   │   └── health.rs     — broken symlinks, missing interpreter, stale lock files
│   ├── actions/          — delete, clear cache, activation cmd, clipboard copy
│   └── tui/
│       ├── app.rs        — AppState: active tab, sort field/dir, selection, scroll
│       ├── ui.rs         — ratatui render functions
│       └── events.rs     — keyboard event loop
```

**Scan flow:** On startup (and manual refresh), rayon walks the root dirs in parallel. Each discovered env is sent via an `mpsc` channel to the main thread, which inserts it into `AppState`. Manager-aware discovery runs concurrently. Health checks are computed lazily after initial population so the table appears quickly with size/path data, health filling in as checks complete.

---

## TUI Layout

```
┌─ clenv ──────────────────────────────────────────────────────────────┐
│ [All] [Python] [Node] [Conda] [Go] [Ruby] [Cargo] [Java]            │
├──────────────────────────────────────────────────────────────────────┤
│ Sort: [Size ▼]  [Name]  [Last Used]  [Health]     Search: _          │
├──────────────────────────────────────────────────────────────────────┤
│ ▶  Name              Path                    Size    Health  Version  │
│ ●  my-project        ~/dev/my-project/.venv  1.2 GB  ✓       3.11.4  │
│    ecom-api          ~/dev/ecom/.venv         340 MB  ⚠       3.10.1  │
│    old-thing         ~/archive/old/.venv      89 MB   ✗       missing │
├──────────────────────────────────────────────────────────────────────┤
│ ┌─ my-project ──────────────────────────────────────────────────────┐ │
│ │ Path:     ~/dev/my-project/.venv                                  │ │
│ │ Packages: 47    Last used: 2 days ago    Cache: 230 MB            │ │
│ │ Health:   OK                                                       │ │
│ │ Activate: source ~/dev/my-project/.venv/bin/activate              │ │
│ └───────────────────────────────────────────────────────────────────┘ │
├──────────────────────────────────────────────────────────────────────┤
│ [d] delete  [c] clear cache  [a] activate  [r] refresh  [?] help    │
└──────────────────────────────────────────────────────────────────────┘
```

- **Tabs** cycle with `Tab` / `Shift+Tab`
- **Sort bar** toggles field and direction with `s`; current field shown with `▲`/`▼`
- **Detail panel** opens below the list for the selected row
- **Search** focuses with `/`, filters by name or path substring

---

## Keybindings

| Key | Action |
|-----|--------|
| `Tab` / `Shift+Tab` | Cycle tabs |
| `↑` / `↓` | Navigate list |
| `s` | Cycle sort field; second press toggles direction |
| `/` | Focus search filter |
| `Esc` | Clear search / close detail panel |
| `d` | Delete selected env (confirmation prompt, shows size freed) |
| `c` | Clear cache subdirs for selected env (no confirmation) |
| `a` | Print activation command to stdout after TUI exits |
| `y` | Copy activation command to clipboard |
| `r` | Refresh — re-run full scan |
| `?` | Toggle help overlay |
| `q` | Quit (saves session state) |

---

## Health Checks

Health is three-tier: `✓` OK, `⚠` Warning, `✗` Broken.

| Check | Tier | Signal |
|-------|------|--------|
| Missing interpreter | Broken | `bin/python` / `bin/node` symlink resolves to nothing |
| Broken symlinks | Warning | Any dangling symlink inside the env dir |
| Stale lock file | Warning | `package-lock.json` / `Pipfile.lock` newer than last install marker |
| Large cache ratio | Warning | Cache subdirs exceed 20% of total env size |
| Unknown version | Warning | Version string cannot be parsed from interpreter |
| Empty env | Warning | Package count is 0 |

---

## Cache Clearing

Each `Environment` carries a `cache_paths: Vec<PathBuf>` listing subdirs that are safe to remove without breaking the env:

| Type | Safe cache paths |
|------|-----------------|
| Python venv | `__pycache__/`, `.cache/`, pip cache |
| node_modules | `.cache/` inside project, npm cache dir |
| Conda | `pkgs/` cache |
| Cargo | `target/debug/`, `target/release/` (not `Cargo.lock`) |
| Go | `$GOPATH/pkg/mod/cache` |

Pressing `c` removes only these paths. Full deletion via `d` removes the entire env root after confirmation.

---

## Configuration

**Location:** `~/.config/clenv/config.toml`

```toml
[scan]
roots = ["~"]
ignore = ["~/Library", "~/.cargo/registry"]
depth_limit = 10

[ui]
default_tab = "All"
default_sort = "size"
default_sort_dir = "desc"

[session]          # written automatically on quit
last_tab = "Python"
last_sort = "size"
last_scroll = 3
```

Config is loaded at startup. If missing, defaults are used and the file is created on first quit. Session state (`[session]`) is written on every clean exit.

---

## Build & Distribution

- **macOS:** standard `cargo build --release` — dynamic links only system libs
- **Linux (Ubuntu):** `cargo build --release --target x86_64-unknown-linux-musl` — fully static binary, no libc dependency
- **No runtime dependencies** beyond the binary itself
- Clipboard support via `arboard` crate (uses system clipboard APIs, no external tools)

---

## Dependencies (planned)

| Crate | Purpose |
|-------|---------|
| `ratatui` | TUI rendering |
| `crossterm` | Terminal backend for ratatui |
| `walkdir` | Recursive filesystem traversal |
| `rayon` | Parallel scanning |
| `clap` | CLI argument parsing |
| `serde` + `toml` | Config file serialization |
| `arboard` | Cross-platform clipboard |
| `dirs` | Platform-correct home/config paths |
| `humansize` | Human-readable size formatting |
| `chrono` | Date/time formatting for last-accessed |
