# clenv

A terminal UI for discovering, inspecting, and cleaning up development environments on your machine — plus a shell config module system for syncing your zsh setup to new machines.

![Rust](https://img.shields.io/badge/built%20with-Rust-orange)
![License](https://img.shields.io/badge/license-MIT-blue)

---

## What it does

`clenv` has two main jobs:

**1. Dev environment manager** — scans your filesystem and surfaces every dev environment it finds (Python virtualenvs, Node `node_modules`, conda/mamba environments, Ruby bundles, Cargo build targets, and more) in a keyboard-driven TUI. See disk usage, check health, copy activation commands, or delete stale environments without leaving the terminal.

**2. Shell config module system** — manage your `~/.zshrc` as a checklist of modules (mamba, zoxide, nvm, oh-my-zsh, etc.). Enable a module to install the binary and inject a fenced block into your shell config. Disable it to remove the block. Reorder blocks with a drag-and-drop interface. Sync to a new machine by checking boxes.

---

## Features

### Dev environment scanning
- **Multi-ecosystem scanning** — detects environments by filesystem structure and via manager CLIs (pyenv, nvm, conda, mamba, micromamba, rbenv, sdkman)
- **Instant startup** — shows cached results immediately while rescanning in the background
- **Health checks** — flags broken symlinks, missing interpreters, stale lock files, and oversized caches
- **Smart deletion** — streamed output, pre-delete preview, and conda-aware cleanup
- **Clickable tab bar** — filter by ecosystem (All / Python / Node / Conda / Go / Ruby / Cargo / Java)
- **Sortable columns** — sort by size, name, last used, or health; click a column to toggle direction
- **Search** — press `/` to filter by name or path
- **Inline detail expansion** — press `Space` to expand package count, cache size, last-used date, and activation command
- **Clipboard support** — copy the activation command for any environment
- **Mouse support** — click tabs, sort labels, scroll wheel navigation

### Shell config module system
- **Checklist-driven setup** — enable/disable zsh tools as a checklist; each module installs its binary and injects a fenced block into `~/.zshrc`
- **15 built-in modules** — oh-my-zsh, powerlevel10k, mamba, conda-aliases, uv, nvm, bun, sdkman, zoxide, thefuck, jabba, homebrew (linux), fzf, fzf-tab, zsh-autosuggestions, fast-syntax-highlighting
- **Block reordering** — File Order page lets you grab and move zshrc blocks interactively
- **Code view** — right-hand panel shows the full `~/.zshrc` with line numbers; selecting a block highlights its lines
- **Adopt unmanaged config** — wrap existing manually-written config in clenv markers without rewriting it
- **Change detection** — background watcher detects external edits to `~/.zshrc` and prompts to reconcile
- **Private dotfiles repo** — sync user customizations from a private git repo
- **AI-assisted onboarding** — `clenv modules --onboard` generates a structured prompt to paste into Claude Code or Claude web

### TUI shell
- **Tab manager** — hide tabs you don't need via the `[⚙]` button
- **Settings overlay** — configure scan roots, UI defaults, and module options without editing TOML by hand
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
clenv                        # launch TUI (uses saved config, or runs first-run wizard)
clenv --root ~/projects      # scan a specific directory
clenv --ignore ~/projects/vendor
```

### Keybindings — Dev environment tab

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
| `a` | Print activation command to stdout |
| `y` | Copy activation command to clipboard |
| `r` | Rescan |
| `?` | Toggle help overlay |
| `,` | Open settings |
| `q` | Quit |

### Keybindings — Shell tab (Modules page)

| Key | Action |
|---|---|
| `↑` / `↓` | Navigate modules |
| `Space` | Toggle module on/off |
| `s` | Save pending changes to `~/.zshrc` |
| `a` | Adopt selected unmanaged block |
| `c` | Copy AI context to clipboard |
| `Tab` / `Shift+Tab` | Switch sub-pages (Modules / File Order) |
| `?` | Help |

### Keybindings — Shell tab (File Order page)

| Key | Action |
|---|---|
| `↑` / `↓` | Navigate blocks |
| `Enter` | Grab / drop block to reorder |
| `◀` / `▶` | Switch sub-pages |

**Mouse:** click a tab or sort label to activate it; scroll wheel to navigate; click `[⚙]` to open the tab manager.

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

---

## Shell module system CLI

All module operations are also available as CLI subcommands, suitable for use with AI agents (Claude Code, etc.):

```sh
clenv modules list              # list all modules with their status
clenv modules show <name>       # show module definition and detect result
clenv modules status            # print enabled modules and .zshrc block health
clenv modules enable <name>     # inject zshrc block (installs binary if missing)
clenv modules disable <name>    # remove zshrc block
clenv modules adopt <name>      # wrap existing unmanaged config in clenv markers
clenv modules verify            # check all enabled modules have correct blocks
clenv modules sync              # pull latest from your private dotfiles repo
clenv modules --onboard         # generate an AI onboarding prompt for Claude
```

### AI onboarding

`clenv modules --onboard` prints a structured prompt (module list + current `.zshrc` content + instructions) that you can paste into Claude Code or Claude web. The AI then uses the CLI commands above to adopt existing config, enable new modules, and verify the result — without any API calls from clenv itself.

For the best experience, run it in Claude Code — the AI has direct terminal access to call `clenv modules` commands immediately.

---

## Configuration

On first run, `clenv` opens an interactive setup wizard. Settings are saved to `~/.config/clenv/config.toml`. You can also edit them via the in-TUI settings overlay (`,` key).

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

[modules]
enabled = ["oh-my-zsh", "powerlevel10k", "mamba", "zoxide"]
zshrc_path = "/Users/you/.zshrc"       # defaults to ~/.zshrc
private_dotfiles_repo = "git@github.com:you/dotfiles.git"  # optional
```

### Config files

| File | Purpose |
|---|---|
| `~/.config/clenv/config.toml` | User settings (scan roots, module state, repo URLs) |
| `~/.config/clenv/cache.json` | Env scan cache — refreshed on every launch |
| `~/.config/clenv/modules/` | User-defined module TOML overrides |
| `~/.config/clenv/private/` | Clone of private dotfiles repo |
| `~/.config/clenv/CLAUDE.md` | Auto-written on init; makes the config dir a Claude Code workspace |

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
