use super::Module;
use std::fs;
use std::path::Path;
use anyhow::Result;

fn open_marker(name: &str) -> String {
    format!("# [clenv: {}]", name)
}

fn close_marker(name: &str) -> String {
    format!("# [/clenv: {}]", name)
}

/// Returns true if ~/.zshrc contains a clenv-managed block for this module.
pub fn has_block(zshrc_path: &Path, name: &str) -> bool {
    let Ok(contents) = fs::read_to_string(zshrc_path) else { return false };
    contents.contains(&open_marker(name))
}

/// Extract the content between clenv markers for this module (not including markers).
pub fn read_block(zshrc_path: &Path, name: &str) -> Option<String> {
    let contents = fs::read_to_string(zshrc_path).ok()?;
    let open = open_marker(name);
    let close = close_marker(name);
    let start = contents.find(&open)?;
    let after_open = start + open.len();
    let end = contents[after_open..].find(&close)?;
    Some(contents[after_open..after_open + end].trim().to_string())
}

/// Insert or replace the fenced block for this module in ~/.zshrc.
/// If a block already exists, it is replaced in-place.
/// If no block exists, the block is appended at the end.
pub fn write_block(zshrc_path: &Path, name: &str, snippet: &str) -> Result<()> {
    let contents = fs::read_to_string(zshrc_path).unwrap_or_default();
    let open = open_marker(name);
    let close = close_marker(name);
    let block = format!("{open} — managed by clenv, do not edit manually\n{snippet}\n{close}\n");

    let new_contents = if contents.contains(&open) {
        // Replace existing block
        let start = contents.find(&open).unwrap();
        let after_open = start + open.len();
        let end_offset = contents[after_open..].find(&close).unwrap();
        let end = after_open + end_offset + close.len();
        // include trailing newline if present
        let end = if contents.as_bytes().get(end) == Some(&b'\n') { end + 1 } else { end };
        format!("{}{}{}", &contents[..start], block, &contents[end..])
    } else {
        format!("{}\n{}", contents.trim_end(), block)
    };

    fs::write(zshrc_path, new_contents)?;
    Ok(())
}

/// Remove the fenced block for this module from ~/.zshrc.
pub fn remove_block(zshrc_path: &Path, name: &str) -> Result<()> {
    let Ok(contents) = fs::read_to_string(zshrc_path) else { return Ok(()) };
    let open = open_marker(name);
    let close = close_marker(name);
    if !contents.contains(&open) {
        return Ok(());
    }
    let start = contents.find(&open).unwrap();
    let after_open = start + open.len();
    let end_offset = contents[after_open..].find(&close).unwrap();
    let end = after_open + end_offset + close.len();
    let end = if contents.as_bytes().get(end) == Some(&b'\n') { end + 1 } else { end };
    let new_contents = format!("{}{}", &contents[..start], &contents[end..]);
    fs::write(zshrc_path, new_contents)?;
    Ok(())
}

/// Heuristic: check if the zshrc mentions keywords suggesting this tool is configured
/// (but not via clenv markers). Used to detect "unmanaged" status.
pub fn has_unmanaged_config(zshrc_path: &Path, module: &Module) -> bool {
    if has_block(zshrc_path, &module.name) {
        return false; // already managed
    }
    let Ok(contents) = fs::read_to_string(zshrc_path) else { return false };
    // Check if any detect command keyword appears in the zshrc
    module.detect.commands.iter().any(|cmd| {
        // Extract the tool name from "which toolname"
        let keyword = cmd.split_whitespace().last().unwrap_or(cmd);
        contents.contains(keyword)
    })
}
