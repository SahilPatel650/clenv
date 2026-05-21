# clenv Module System — Agent Context

## What the Module System Does

`clenv modules` manages shell configuration for zsh tools. Each module:
1. **Detects** whether the tool is installed (via shell commands)
2. **Injects** a fenced block into `~/.zshrc` with the correct initialization snippet
3. **Tracks** enabled/disabled state in `~/.config/clenv/config.toml`

Enabling a module writes a fenced block to `~/.zshrc`. Disabling removes it. The binary is never uninstalled automatically.

---

## Full TOML Schema

```toml
[module]
name        = "nvm"                          # unique identifier, used in CLI commands
description = "Node Version Manager"         # shown in clenv modules list/show
category    = "node"                         # grouping label (package-managers, shell-frameworks, node, etc.)

[detect]
commands = ["which nvm", "test -d ~/.nvm"]   # any command that exits 0 means "installed"

[install.linux]
command = "curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.7/install.sh | bash"

[install.macos]
command = "brew install nvm"

[zshrc]
snippet = """
export NVM_DIR="$HOME/.nvm"
[ -s "$NVM_DIR/nvm.sh" ] && \\. "$NVM_DIR/nvm.sh"  # lazy load
"""
startup_ms_estimate = 250   # milliseconds this snippet adds to shell startup; shown in Shell tab
order = 200                 # injection order in .zshrc (lower = earlier); powerlevel10k must be 0
user_extend = "modules/nvm-custom.zsh"  # path relative to ~/.config/clenv/; sourced if present

[[depends_on]]
module = "oh-my-zsh"   # clenv ensures this module is enabled first
```

All fields under `[install]` are optional. If the platform key is absent, `clenv modules enable` will inject the zshrc block but skip the install step.

---

## CLI Commands

```
clenv modules list                 List all modules with name, category, and status
clenv modules show <name>          Full detail: description, detect commands, install command, snippet
clenv modules status               Summary: which modules are enabled and block presence
clenv modules enable <name>        Inject zshrc block + add to enabled list in config
clenv modules disable <name>       Remove zshrc block + remove from enabled list
clenv modules adopt <name>         Write managed block (wrapping existing unmanaged config)
clenv modules verify               Check every enabled module has a correct block in .zshrc
clenv modules --onboard            Print AI onboarding prompt to stdout (includes this file + current state)
```

### Examples

```sh
# See what's available
clenv modules list

# Check a specific module
clenv modules show nvm

# Manage a module
clenv modules enable zoxide
clenv modules disable thefuck
clenv modules adopt oh-my-zsh    # you already have oh-my-zsh configured manually

# Verify nothing is broken
clenv modules verify
```

---

## Fenced Block Format

Every managed snippet is wrapped with clenv markers:

```zsh
# [clenv: nvm] — managed by clenv, do not edit manually
export NVM_DIR="$HOME/.nvm"
[ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"
# [/clenv: nvm]
```

Rules:
- The open marker must appear verbatim for `has_block` / `read_block` / `remove_block` to work
- Do not manually edit content between the markers — use `clenv modules enable <name>` to update
- The entire block (markers + content) is atomic: remove_block deletes everything between and including the markers

---

## Performance Rules for zshrc Snippets

These rules keep shell startup fast. Violating them causes noticeable lag.

1. **Do not call `compinit` more than once.** Each call adds 200–600 ms.
2. **Use lazy loading for heavy tools.** NVM, thefuck, and similar tools should not run unconditionally at startup.
3. **Avoid unconditional `eval` of slow commands.** Prefer precomputed init strings or conditional checks.
   - BAD:  `eval "$(thefuck --alias)"`  ← runs thefuck on every shell start
   - GOOD: lazy-load wrapper that runs `thefuck --alias` only when first invoked
4. **Set `startup_ms_estimate`** in every module TOML. The Shell tab uses this to show total overhead.
5. **`powerlevel10k` instant prompt must be first.** Set `order = 0` for powerlevel10k.
6. **Source order matters.** `depends_on` ensures prerequisite modules appear earlier in `.zshrc`.

---

## AI Workflow for Onboarding

When you run `clenv modules --onboard`, the output is designed to be pasted into Claude Code or Claude web. The recommended workflow:

1. Review `clenv modules list` output to understand current state
2. For tools already in `.zshrc` but unmanaged: run `clenv modules adopt <name>`
3. For new tools you want: run `clenv modules enable <name>`
4. Confirm everything is correct: `clenv modules verify`
5. Open a new shell to test startup time

The AI should NOT directly edit `~/.zshrc`. All `.zshrc` changes must go through `clenv modules` CLI commands.
