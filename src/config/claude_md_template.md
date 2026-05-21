# clenv Shell Config — Claude Code Workspace

This directory (~/.config/clenv/) is managed by [clenv](https://github.com/SahilPatel650/clenv).

## What this workspace is for

Use Claude Code in this directory to manage your zsh shell configuration via clenv modules.
Each module represents a tool (mamba, zoxide, nvm, etc.) and controls:
- Whether it's installed on this machine
- What snippet is injected into ~/.zshrc

## Available CLI commands

| Command | Description |
|---|---|
| `clenv modules list` | List all modules with current status |
| `clenv modules show <name>` | Show module details and detect result |
| `clenv modules enable <name>` | Enable a module (installs + injects zshrc block) |
| `clenv modules disable <name>` | Disable a module (removes zshrc block) |
| `clenv modules adopt <name>` | Adopt existing unmanaged config under clenv management |
| `clenv modules verify` | Check all enabled modules have correct blocks |
| `clenv modules --onboard` | Generate a full AI onboarding prompt |

## Module TOML schema

Built-in modules live in the clenv binary. User-defined overrides go in `~/.config/clenv/modules/`.

```toml
name = "my-tool"
description = "What this tool does"
category = "productivity"        # package-managers | shell-frameworks | shell-themes | zsh-plugins | productivity | aliases

[detect]
commands = ["which my-tool"]

[install.linux]
command = "apt install my-tool"

[install.macos]
command = "brew install my-tool"

[zshrc]
snippet = """
eval "$(my-tool init zsh)"
"""
startup_ms_estimate = 30    # estimated ms added to shell startup
order = 100                 # injection order in .zshrc (lower = earlier)
user_extend = "modules/my-tool-custom.zsh"   # sourced from private/ if present

[[depends_on]]
module = "other-module"
```

## zshrc block format

clenv manages blocks in ~/.zshrc with fenced comment markers:

```zsh
# [clenv: module-name] — managed by clenv, do not edit manually
...snippet...
# [/clenv: module-name]
```

Do not edit the content between markers manually — use `clenv modules adopt` or `clenv modules enable` instead.

## Performance rules

- **Never call `compinit` more than once** — it adds 200–600ms startup time
- **Lazy-load heavy tools** (NVM, thefuck) — defer init until first use
- **Avoid unconditional `eval` of slow commands** — precompute or use conditional checks
- **Set `startup_ms_estimate`** in every module so the Shell tab can show overhead
- `powerlevel10k` instant prompt must have `order = 0` (before everything else)

## Private dotfiles repo

Set `private_dotfiles_repo` in `~/.config/clenv/config.toml` to a git URL.
clenv will clone/pull it into `~/.config/clenv/private/`.
Modules with `user_extend` will source the matching file from `private/` if it exists.
