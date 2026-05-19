# clenv

A terminal UI for discovering, inspecting, and cleaning up development environments on your machine.

![Rust](https://img.shields.io/badge/built%20with-Rust-orange)
![License](https://img.shields.io/badge/license-MIT-blue)

---

## What it does

`clenv` scans your filesystem and surfaces every dev environment it finds — Python virtualenvs, Node `node_modules`, conda/mamba environments, Ruby bundles, Cargo build targets, and more — in a keyboard-driven TUI. You can see how much disk space each one takes, check its health, copy its activation command, or delete it to free up space.

---

## Features

- **Multi-ecosystem scanning** — detects environments by filesystem structure and via manager CLIs (pyenv, nvm, conda, mamba, micromamba, rbenv, sdkman)
- **Instant startup** — shows cached results immediately while rescanning in the background
- **Health checks** — flags broken symlinks, missing interpreters, stale lock files, and oversized caches
- **Clickable tab bar** — filter by ecosystem (All / Python / Node / Conda / Go / Ruby / Cargo / Java)
- **Sortable columns** — sort by size, name, last used, or health; click a column to toggle direction
- **Search** — press `/` to filter by name or path, with full space support in queries
- **Inline detail expansion** — press `Space` on any row to expand package count, cache size, last-used date, and activation command
- **Delete & cache-clear** — reclaim disk space without leaving the terminal
- **Clipboard support** — copy the activation command for any environment
- **Mouse support** — click tabs, sort labels, and the tab manager; scroll with the wheel
- **Tab manager** — hide tabs you don't need via the `[⚙]` button
- **Session persistence** — remembers your last active tab, sort order, and scroll position
- **First-run wizard** — guided TUI setup on first launch with path autocomplete

---

## Supported environment types

| Type | Detected by |
|---|---|
| Python virtualenv | `pyvenv.cfg` present |
| Node `node_modules` | directory named `node_modules` next to `package.json` |
| Conda / Mamba env | `conda-meta/` directory; also via `conda`, `mamba`, `micromamba` CLI |
| Ruby bundle | `.bundle/gems/` directory |
| Cargo build target | `target/CACHEDIR.TAG` present |
| Go workspace | *(via filesystem marker)* |
| Java (sdkman) | via `sdk` CLI |
| pyenv versions | via `pyenv` CLI |
| nvm versions | via `nvm` |
| rbenv versions | via `rbenv` CLI |

---

## Installation

### From source

```sh
git clone https://github.com/SahilPatel650/clenv
cd clenv
cargo install --path .
```

Requires Rust 1.75+. Install via [rustup](https://rustup.rs) if needed.

---

## Usage

```sh
clenv                        # scan using saved config (or run the first-run wizard)
clenv --root ~/projects      # scan a specific directory
clenv --ignore ~/projects/vendor --ignore ~/projects/.cache
```

### Keybindings

| Key | Action |
|---|---|
| `Tab` / `Shift+Tab` | Cycle through visible tabs |
| `↑` / `↓` or `k` / `j` | Navigate the list |
| `PgUp` / `PgDn` | Jump 10 rows |
| `Space` | Expand / collapse environment details |
| `/` | Enter search mode |
| `Esc` | Exit search / dismiss messages |
| `s` | Cycle sort field |
| `d` | Delete selected environment |
| `c` | Clear environment cache directory |
| `a` | Print activation command to stdout (for shell eval) |
| `y` | Copy activation command to clipboard |
| `r` | Rescan now |
| `?` | Toggle help overlay |
| `q` | Quit |

**Mouse:** click a tab or sort label to activate it; click the same sort label again to reverse direction; scroll wheel to navigate the list; click `[⚙]` to open the tab manager.

### Shell activation

To activate an environment directly from `clenv`, wrap it in a shell function:

```sh
# bash / zsh
function ce() {
  local cmd
  cmd=$(clenv "$@")
  [ -n "$cmd" ] && eval "$cmd"
}
```

Then press `a` on any environment and run `ce` to activate it in your current shell.

---

## Configuration

On first run, `clenv` opens an interactive setup wizard. Settings are saved to:

```
~/.config/clenv/config.toml
```

Example config:

```toml
[scan]
roots = ["/Users/you/projects", "/Users/you/work"]
ignore = ["/Users/you/projects/archived"]
depth_limit = 10

[ui]
default_tab = "All"
default_sort = "size"
default_sort_dir = "desc"

[session]
last_tab = "Python"
last_sort = "size"
last_scroll = 0
```

The scan cache is stored at `~/.config/clenv/cache.json` and is refreshed on every launch.

---

## Building

```sh
cargo build --release
# binary at ./target/release/clenv
```

```sh
cargo test
```

---

## License

MIT
