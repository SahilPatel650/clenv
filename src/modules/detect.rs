use super::{BlockDiff, DiffLine, DiffLineKind, Module, ModuleStatus};
use super::zshrc;
use similar::{ChangeTag, TextDiff};
use std::path::Path;
use std::process::Command;

/// Returns true if any of the module's detect commands succeed (exit code 0).
pub fn is_installed(module: &Module) -> bool {
    if module.detect.commands.is_empty() {
        return false;
    }
    module.detect.commands.iter().any(|cmd| {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if parts.is_empty() { return false; }
        Command::new(parts[0])
            .args(&parts[1..])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

/// Returns true if the module has an install command for the current platform.
pub fn has_install_for_platform(module: &Module) -> bool {
    let Some(install) = &module.install else { return false; };
    #[cfg(target_os = "macos")]
    return install.macos.is_some();
    #[cfg(not(target_os = "macos"))]
    return install.linux.is_some();
}

/// Returns names of `depends_on` modules that are not currently installed.
pub fn missing_deps<'a>(module: &'a Module, all_modules: &[Module]) -> Vec<String> {
    module.depends_on.iter()
        .filter(|dep_name| {
            match all_modules.iter().find(|m| &m.name == *dep_name) {
                Some(dep) => !is_installed(dep),
                None => true,
            }
        })
        .cloned()
        .collect()
}

/// Compare the canonical TOML snippet against what is currently in .zshrc.
/// Returns None if the content is identical or the block is not present.
pub fn compute_block_diff(canonical: &str, current: &str) -> Option<BlockDiff> {
    let a = canonical.trim();
    let b = current.trim();
    if a == b {
        return None;
    }

    let text_diff = TextDiff::from_lines(a, b);
    let changes: Vec<_> = text_diff.iter_all_changes().collect();

    let mut lines: Vec<DiffLine> = Vec::new();
    let mut i = 0usize;

    while i < changes.len() {
        let ch = &changes[i];
        match ch.tag() {
            ChangeTag::Equal => {
                let text = ch.value().trim_end_matches('\n').to_string();
                lines.push(DiffLine {
                    kind: DiffLineKind::Equal,
                    spans: vec![(text, false)],
                });
                i += 1;
            }
            ChangeTag::Delete => {
                // Look ahead: if next change is an Insert, do word-level diff
                if i + 1 < changes.len() && changes[i + 1].tag() == ChangeTag::Insert {
                    let old = changes[i].value().trim_end_matches('\n');
                    let new = changes[i + 1].value().trim_end_matches('\n');
                    let wd = TextDiff::from_words(old, new);
                    let mut rem_spans: Vec<(String, bool)> = Vec::new();
                    let mut add_spans: Vec<(String, bool)> = Vec::new();
                    for wch in wd.iter_all_changes() {
                        let t = wch.value().to_string();
                        match wch.tag() {
                            ChangeTag::Equal => {
                                rem_spans.push((t.clone(), false));
                                add_spans.push((t, false));
                            }
                            ChangeTag::Delete => rem_spans.push((t, true)),
                            ChangeTag::Insert => add_spans.push((t, true)),
                        }
                    }
                    lines.push(DiffLine { kind: DiffLineKind::Removed, spans: rem_spans });
                    lines.push(DiffLine { kind: DiffLineKind::Added, spans: add_spans });
                    i += 2;
                } else {
                    let text = ch.value().trim_end_matches('\n').to_string();
                    lines.push(DiffLine {
                        kind: DiffLineKind::Removed,
                        spans: vec![(text, true)],
                    });
                    i += 1;
                }
            }
            ChangeTag::Insert => {
                let text = ch.value().trim_end_matches('\n').to_string();
                lines.push(DiffLine {
                    kind: DiffLineKind::Added,
                    spans: vec![(text, true)],
                });
                i += 1;
            }
        }
    }

    Some(BlockDiff { lines })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_block_diff_returns_none_when_identical() {
        let snippet = "export FOO=bar";
        assert!(compute_block_diff(snippet, snippet).is_none());
    }

    #[test]
    fn compute_block_diff_with_custom_source_uses_provided_canonical() {
        let custom = "export FOO=custom";
        let current = "export FOO=different";
        let diff = compute_block_diff(custom, current);
        assert!(diff.is_some());
    }
}

/// Derive the full ModuleStatus for a module.
pub fn module_status(module: &Module, zshrc_path: &Path) -> ModuleStatus {
    let installed = is_installed(module);
    let has_block = zshrc::has_block(zshrc_path, &module.name);

    match (installed, has_block) {
        (_, true) => ModuleStatus::ManagedActive,
        (true, false) => ModuleStatus::ManagedInactive,
        (false, false) => ModuleStatus::NotInstalled,
    }
}
