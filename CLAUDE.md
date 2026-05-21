# clenv — Claude Code Guide

## Project Overview

`clenv` is a Rust TUI app (ratatui + crossterm) for discovering, inspecting, and cleaning up development environments (Python venvs, conda, Node, Go, Ruby, etc.). It scans the filesystem in a background thread, caches results, and lets users manage environments interactively.

## Codebase Layout

```
src/
├── main.rs              — entry point, CLI args (clap), background scan launch
├── lib.rs               — public re-exports (actions, config, env, scanner)
├── tui/
│   ├── mod.rs           — TUI event loop, terminal setup/teardown
│   ├── app.rs           — AppState struct, Tab/SortField enums, filtering/sorting
│   ├── ui.rs            — all rendering functions (ratatui)
│   ├── theme.rs         — Theme struct; always use theme fields, never hardcode Color::*
│   ├── events.rs        — key/mouse handlers → EventOutcome enum
│   └── onboarding.rs    — first-run setup wizard state machine
├── env/
│   ├── mod.rs           — Environment, EnvKind, HealthStatus structs
│   ├── metrics.rs       — size, package count, version detection
│   └── health.rs        — health check implementations
├── scanner/
│   ├── mod.rs           — 4-phase scan pipeline (fs walk → enrich → managers → dedup)
│   ├── fs.rs            — filesystem heuristics for env detection
│   └── managers/        — per-tool adapters (conda, nvm, pyenv, etc.)
├── actions/
│   └── mod.rs           — delete strategies, cache clear, clipboard copy
├── config/
│   ├── mod.rs           — Config struct (scan, ui, session, modules), load/save to ~/.config/clenv/config.toml
│   └── cache.rs         — env scan cache → ~/.config/clenv/cache.json
└── modules/             — (planned) shell config module system
    ├── mod.rs           — Module struct, ModuleStatus enum, registry
    ├── zshrc.rs         — fenced block injection into ~/.zshrc
    ├── detect.rs        — binary/config presence detection
    ├── installer.rs     — OS-specific install command runner (streaming output)
    ├── resolver.rs      — topological dependency sort
    └── builtin/         — module TOML definitions shipped with the binary
```

## Key Patterns

### Adding a New Tab

1. Add variant to `Tab` enum in `src/tui/app.rs`
2. Add to `Tab::ALL` array
3. Add `label()` match arm
4. Add `matches_kind()` match arm (or `true` for a content-specific tab like Shell)
5. HitRect click handling is automatic — no extra registration

### Adding a New Overlay/Popup

1. Add a `bool` or state struct to `AppState`
2. Write `render_my_overlay(frame, app, area, theme)` in `ui.rs`
3. Call it conditionally from `render()`, after other overlays
4. Use `popup_block(title, theme)` helper for consistent border/bg styling
5. Add key handlers in `events.rs` (new branch in `handle_key`)
6. Store `HitRect`s in AppState for clickable items inside the popup

### Streaming Output (TUI Suspend/Resume)

When an action streams subprocess output, suspend the TUI:
```rust
disable_raw_mode();
execute!(stdout, LeaveAlternateScreen, DisableMouseCapture);
terminal.show_cursor();
// ... run the command ...
enable_raw_mode();
execute!(stdout, EnterAlternateScreen, EnableMouseCapture);
terminal.clear();
```
See `EventOutcome::DeleteConfirmed` handling in `src/tui/mod.rs` for the full pattern.

## Theming Rules

**Always use `theme.*` fields — never hardcode `Color::*` in render functions.**

```rust
// WRONG
Span::styled("foo", Style::default().fg(Color::Yellow))

// RIGHT
Span::styled("foo", Style::default().fg(theme.accent))
```

The `Theme` struct and `default_theme()` live in `src/tui/theme.rs`. The theme is created once in `render()` and passed as `&Theme` to every `render_*` helper. Use the `popup_block(title, theme)` helper for all overlay/popup `Block` widgets.

Semantic color names:
- `theme.accent`    — Yellow; active tab, table header, emphasis
- `theme.highlight` — Cyan; labels, active input cursor, sort selection
- `theme.ok`        — Green; healthy status, success messages
- `theme.warn`      — Yellow; warning health status
- `theme.danger`    — Red; broken health, delete confirm border
- `theme.muted`     — DarkGray; secondary/inactive text
- `theme.text`      — White; primary text in popups
- `theme.popup_bg`  — Black; overlay background

## Shell Module System (Planned — `src/modules/`)

Each zsh tool (mamba, zoxide, nvm, etc.) is a TOML file in `src/modules/builtin/`. Modules are embedded in the binary via `include_str!()` at build time; user-defined overrides go in `~/.config/clenv/modules/`.

Enabling a module: installs binary (OS-specific command) → injects fenced zshrc block.
Disabling: removes the fenced block (binary untouched by default).

### .zshrc Block Format
```zsh
# [clenv: module-name] — managed by clenv, do not edit manually
...snippet...
# [/clenv: module-name]
```

### Module TOML Schema
```toml
[module]
name = "mamba"
description = "Fast conda-compatible package manager"
category = "package-managers"

[detect]
commands = ["which mamba", "which micromamba"]

[install.linux]
command = "..."

[install.macos]
command = "brew install micromamba"

[zshrc]
snippet = "..."
startup_ms_estimate = 50   # shown in Shell tab detail panel
order = 100                # injection order in .zshrc (lower = earlier)
user_extend = "modules/mamba-custom.zsh"  # relative to ~/.config/clenv/; sourced if present

[[depends_on]]
module = "conda-aliases"
```

### CLI Tools for AI Agents
```
clenv modules list              # all modules with status
clenv modules show <name>       # definition + detect result
clenv modules enable <name>     # install + inject block
clenv modules disable <name>    # remove block (prompts for binary uninstall)
clenv modules adopt <name>      # wrap existing unmanaged config in clenv markers
clenv modules verify            # check all enabled blocks are present and correct
clenv modules --onboard         # generate prompt for Claude Code / Claude web
```

### Performance Rules for zshrc Snippets
- **Do not call `compinit` more than once.** It adds 200-600ms.
- **Use lazy loading for heavy tools** (NVM, thefuck, etc.).
- **Avoid unconditional `eval` of slow commands** (prefer precomputed init or conditional check).
- **Set `startup_ms_estimate`** in every module TOML so the Shell tab can show overhead.
- `powerlevel10k` instant prompt must be first in .zshrc (order = 0).

## Config Files

| File | Purpose |
|---|---|
| `~/.config/clenv/config.toml` | User settings (scan roots, module state, repo URLs) |
| `~/.config/clenv/cache.json` | Env scan cache (loaded instantly on startup) |
| `~/.config/clenv/modules/` | User-defined module TOML overrides |
| `~/.config/clenv/private/` | Clone of private dotfiles repo (user customizations) |
| `~/.config/clenv/agents/` | Clone of agent context repo (AI prompt files) |
| `~/.config/clenv/CLAUDE.md` | Auto-written on init; makes config dir a Claude Code workspace |
