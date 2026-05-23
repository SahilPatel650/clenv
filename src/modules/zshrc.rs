use super::Module;
use std::fs;
use std::path::Path;
use anyhow::Result;

// ── Marker helpers ────────────────────────────────────────────────────────────

fn open_marker(name: &str) -> String {
    format!("# [clenv: {}]", name)
}

fn close_marker(name: &str) -> String {
    format!("# [/clenv: {}]", name)
}

// Header lines that precede the open marker (written by write_block).
// Used by remove_block to strip the whole block including its header.
fn header_prefix() -> &'static str {
    "# clenv>"
}

// ── Segment types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ZshrcSegment {
    pub kind: SegmentKind,
    pub content: String,
}

#[derive(Debug, Clone)]
pub enum SegmentKind {
    /// A clenv-managed block. `name` is the module name.
    Clenv(String),
    /// Content between (or before/after) clenv blocks that is not managed by clenv.
    Unmanaged,
}

/// Parse ~/.zshrc into ordered segments: clenv blocks and unmanaged content.
/// `# clenv>` header lines that immediately precede a clenv open marker are
/// silently excluded from the returned Unmanaged content (they belong to the
/// following Clenv segment's metadata).
pub fn parse_segments(zshrc_path: &Path) -> Vec<ZshrcSegment> {
    let Ok(contents) = fs::read_to_string(zshrc_path) else {
        return vec![];
    };
    let mut segments: Vec<ZshrcSegment> = Vec::new();
    let mut unmanaged_lines: Vec<&str> = Vec::new();
    let mut in_block: Option<String> = None;
    let mut block_lines: Vec<&str> = Vec::new();

    for line in contents.lines() {
        if let Some(ref name) = in_block.clone() {
            if line == close_marker(name) {
                flush_unmanaged(&mut unmanaged_lines, &mut segments);
                segments.push(ZshrcSegment {
                    kind: SegmentKind::Clenv(name.clone()),
                    content: block_lines.join("\n"),
                });
                block_lines.clear();
                in_block = None;
            } else {
                block_lines.push(line);
            }
        } else if let Some(name) = try_parse_open_marker(line) {
            in_block = Some(name);
        } else if line.trim_start().starts_with(header_prefix()) {
            // Header comment preceding a clenv block — don't include in unmanaged
            // but don't flush yet; if no clenv open follows, add them back below
            unmanaged_lines.push(line);
        } else {
            unmanaged_lines.push(line);
        }
    }

    // Remaining content after last block
    flush_unmanaged(&mut unmanaged_lines, &mut segments);
    segments
}

/// Returns `(start_line, end_line)` pairs (1-indexed, inclusive) for each segment
/// produced by [`parse_segments`]. Index `i` in the returned vec corresponds to
/// `segments[i]` from that function, so both vecs are always the same length.
///
/// For Clenv segments the range spans the open marker through the close marker.
/// For Unmanaged segments it covers all non-empty lines in the region.
pub fn segment_line_ranges(path: &Path) -> Vec<(usize, usize)> {
    let Ok(contents) = fs::read_to_string(path) else {
        return vec![];
    };
    let pfx = header_prefix();
    let mut ranges: Vec<(usize, usize)> = Vec::new();

    // Pending unmanaged region state (0 = no region open yet)
    let mut um_start: usize = 0;
    let mut um_end: usize = 0;
    let mut um_has_content = false;

    let mut in_block: Option<(String, usize)> = None; // (module-name, start-line)
    let mut line_num: usize = 1;

    for line in contents.lines() {
        if let Some((ref name, block_start)) = in_block.clone() {
            if line == close_marker(name) {
                // Flush any pending unmanaged that preceded this block
                if um_has_content {
                    ranges.push((um_start, um_end));
                }
                um_start = 0;
                um_end = 0;
                um_has_content = false;
                ranges.push((block_start, line_num));
                in_block = None;
            }
            // Lines inside a block don't affect the unmanaged region
        } else if let Some(name) = try_parse_open_marker(line) {
            in_block = Some((name, line_num));
        } else {
            // Unmanaged or header-prefix line — extend the pending unmanaged region
            if um_start == 0 {
                um_start = line_num;
            }
            um_end = line_num;
            if !line.trim_start().starts_with(pfx) && !line.trim().is_empty() {
                um_has_content = true;
            }
        }
        line_num += 1;
    }

    // Flush any trailing unmanaged content
    if um_has_content {
        ranges.push((um_start, um_end));
    }

    ranges
}

/// Extract module name from an open marker line (handles old and new format).
fn try_parse_open_marker(line: &str) -> Option<String> {
    // Matches "# [clenv: <name>]" optionally followed by more text (old format had " — managed…")
    let rest = line.strip_prefix("# [clenv: ")?;
    let name = rest.split(']').next()?;
    if name.is_empty() { return None; }
    Some(name.to_string())
}

fn flush_unmanaged<'a>(lines: &mut Vec<&'a str>, segments: &mut Vec<ZshrcSegment>) {
    if lines.is_empty() { return; }
    let pfx = header_prefix();
    let filtered: Vec<&str> = lines.iter()
        .copied()
        .filter(|l| !l.trim_start().starts_with(pfx))
        .collect();
    let content = filtered.join("\n");
    let has_content = content.lines().any(|l| !l.trim().is_empty());
    if has_content {
        segments.push(ZshrcSegment {
            kind: SegmentKind::Unmanaged,
            content: content.trim_end().to_string(),
        });
    }
    lines.clear();
}

// ── Block read ────────────────────────────────────────────────────────────────

/// Returns true if ~/.zshrc contains a clenv-managed block for this module.
pub fn has_block(zshrc_path: &Path, name: &str) -> bool {
    let Ok(contents) = fs::read_to_string(zshrc_path) else { return false };
    contents.contains(&open_marker(name))
}

/// Extract the snippet content between clenv markers.
/// Strips the old-format "— managed by clenv" trailing text on the open marker line.
pub fn read_block(zshrc_path: &Path, name: &str) -> Option<String> {
    let contents = fs::read_to_string(zshrc_path).ok()?;
    let open = open_marker(name);
    let close = close_marker(name);
    let start = contents.find(&open)?;
    let after_open = start + open.len();
    let end = contents[after_open..].find(&close)?;
    let raw = contents[after_open..after_open + end].trim().to_string();

    // Strip old-format header line (e.g. "— managed by clenv, do not edit manually")
    let first_line_is_meta = raw.lines().next()
        .map_or(false, |l| l.trim_start_matches('—').trim().starts_with("managed by clenv"));
    if first_line_is_meta {
        Some(raw.lines().skip(1).collect::<Vec<_>>().join("\n").trim().to_string())
    } else {
        Some(raw)
    }
}

/// Heuristic: check if the zshrc mentions keywords suggesting this tool is configured
/// (but not via clenv markers). Used to detect "unmanaged" status.
pub fn has_unmanaged_config(zshrc_path: &Path, module: &Module) -> bool {
    if has_block(zshrc_path, &module.name) {
        return false; // already managed
    }
    let Ok(contents) = fs::read_to_string(zshrc_path) else { return false };
    module.detect.commands.iter().any(|cmd| {
        let keyword = cmd.split_whitespace().last().unwrap_or(cmd);
        contents.contains(keyword)
    })
}

// ── Block write / remove ──────────────────────────────────────────────────────

/// Find the byte offset where the full block (including preceding # clenv> header
/// lines) begins. Falls back to the open marker position if no header is present.
fn find_block_full_start(contents: &str, name: &str) -> usize {
    let open = open_marker(name);
    let Some(marker_pos) = contents.find(&open) else { return contents.len() };

    let text_before = &contents[..marker_pos];
    let pfx = header_prefix();

    // Collect (byte_offset, line_text) pairs
    let mut lines: Vec<(usize, &str)> = Vec::new();
    let mut pos = 0usize;
    for line in text_before.split('\n') {
        lines.push((pos, line));
        pos += line.len() + 1;
    }
    // Trailing empty split artefact from split('\n') — pop if empty
    if lines.last().map_or(false, |&(_, l)| l.is_empty()) {
        lines.pop();
    }

    let mut start_offset = marker_pos;
    for &(offset, line) in lines.iter().rev() {
        if line.trim_start().starts_with(pfx) {
            start_offset = offset;
        } else {
            break;
        }
    }
    start_offset
}

/// Insert or replace the fenced block for this module in ~/.zshrc.
pub fn write_block(zshrc_path: &Path, name: &str, snippet: &str) -> Result<()> {
    let contents = fs::read_to_string(zshrc_path).unwrap_or_default();
    let block = build_block(name, snippet);

    let new_contents = if contents.contains(&open_marker(name)) {
        let start = find_block_full_start(&contents, name);
        let open = open_marker(name);
        let close = close_marker(name);
        let marker_pos = contents.find(&open).unwrap();
        let after_marker = marker_pos + open.len();
        let close_offset = contents[after_marker..].find(&close).unwrap();
        let end = after_marker + close_offset + close.len();
        let end = if contents.as_bytes().get(end) == Some(&b'\n') { end + 1 } else { end };
        format!("{}{}{}", &contents[..start], block, &contents[end..])
    } else {
        format!("{}\n{}", contents.trim_end(), block)
    };

    fs::write(zshrc_path, new_contents)
        .map_err(anyhow::Error::from)
}

/// Write a new block inserted immediately after `after_block` (or append if None).
/// Used for user-created custom blocks.
pub fn write_block_at(
    zshrc_path: &Path,
    name: &str,
    snippet: &str,
    after_block: Option<&str>,
) -> Result<()> {
    let contents = fs::read_to_string(zshrc_path).unwrap_or_default();
    // Don't write if already exists
    if contents.contains(&open_marker(name)) {
        return write_block(zshrc_path, name, snippet);
    }
    let block = build_block(name, snippet);

    let new_contents = if let Some(prev) = after_block {
        let close = close_marker(prev);
        if let Some(pos) = contents.find(&close) {
            let end = pos + close.len();
            let end = if contents.as_bytes().get(end) == Some(&b'\n') { end + 1 } else { end };
            format!("{}\n{}{}", &contents[..end].trim_end(), block, &contents[end..])
        } else {
            format!("{}\n{}", contents.trim_end(), block)
        }
    } else {
        format!("{}\n{}", contents.trim_end(), block)
    };

    fs::write(zshrc_path, new_contents)?;
    Ok(())
}

/// Remove the fenced block for this module from ~/.zshrc, including any
/// preceding `# clenv>` header lines.
pub fn remove_block(zshrc_path: &Path, name: &str) -> Result<()> {
    let Ok(contents) = fs::read_to_string(zshrc_path) else { return Ok(()) };
    if !contents.contains(&open_marker(name)) {
        return Ok(());
    }
    let start = find_block_full_start(&contents, name);
    let open = open_marker(name);
    let close = close_marker(name);
    let marker_pos = contents.find(&open).unwrap();
    let after_marker = marker_pos + open.len();
    let close_offset = contents[after_marker..].find(&close).unwrap();
    let end = after_marker + close_offset + close.len();
    let end = if contents.as_bytes().get(end) == Some(&b'\n') { end + 1 } else { end };

    let new_contents = format!("{}{}", &contents[..start], &contents[end..]);
    fs::write(zshrc_path, new_contents)?;
    Ok(())
}

/// Reorder segments by moving the segment at `from_idx` to `to_idx`.
/// Indices are into the `parse_segments()` result (0-based).
/// Returns `Err` if either index is out of bounds.
pub fn move_block(path: &Path, from_idx: usize, to_idx: usize) -> anyhow::Result<()> {
    let mut segments = parse_segments(path);
    if from_idx >= segments.len() || to_idx >= segments.len() {
        anyhow::bail!(
            "move_block: index out of bounds (from={from_idx}, to={to_idx}, len={})",
            segments.len()
        );
    }
    if from_idx == to_idx {
        return Ok(());
    }
    let seg = segments.remove(from_idx);
    segments.insert(to_idx, seg);

    let mut parts: Vec<String> = Vec::new();
    for seg in &segments {
        match &seg.kind {
            SegmentKind::Clenv(name) => {
                parts.push(build_block(name, &seg.content));
            }
            SegmentKind::Unmanaged => {
                if !seg.content.is_empty() {
                    parts.push(format!("{}\n", seg.content));
                }
            }
        }
    }
    let new_content = parts.join("");
    std::fs::write(path, new_content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    fn write_tmp(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    #[test]
    fn move_block_swaps_two_blocks() {
        let content = "# [clenv: a]\nA_CONTENT\n# [/clenv: a]\n# [clenv: b]\nB_CONTENT\n# [/clenv: b]\n";
        let f = write_tmp(content);
        move_block(f.path(), 0, 1).unwrap();
        let result = std::fs::read_to_string(f.path()).unwrap();
        let pos_b = result.find("# [clenv: b]").unwrap();
        let pos_a = result.find("# [clenv: a]").unwrap();
        assert!(pos_b < pos_a, "b should appear before a after move");
    }

    #[test]
    fn move_block_noop_same_index() {
        let content = "# [clenv: a]\nA\n# [/clenv: a]\n# [clenv: b]\nB\n# [/clenv: b]\n";
        let f = write_tmp(content);
        move_block(f.path(), 0, 0).unwrap();
        let result = std::fs::read_to_string(f.path()).unwrap();
        assert!(result.contains("# [clenv: a]"));
        assert!(result.contains("# [clenv: b]"));
    }

    #[test]
    fn move_block_last_to_first() {
        let content = "# [clenv: a]\nA\n# [/clenv: a]\n# [clenv: b]\nB\n# [/clenv: b]\n# [clenv: c]\nC\n# [/clenv: c]\n";
        let f = write_tmp(content);
        move_block(f.path(), 2, 0).unwrap();
        let result = std::fs::read_to_string(f.path()).unwrap();
        let pos_c = result.find("# [clenv: c]").unwrap();
        let pos_a = result.find("# [clenv: a]").unwrap();
        assert!(pos_c < pos_a, "c should be first after move");
    }

    #[test]
    fn move_block_out_of_bounds_returns_err() {
        let content = "# [clenv: a]\nA\n# [/clenv: a]\n";
        let f = write_tmp(content);
        assert!(move_block(f.path(), 0, 5).is_err());
    }

    #[test]
    fn parse_segments_two_managed_one_unmanaged() {
        let zshrc = write_tmp(
            "export PATH=\"$HOME/.local/bin:$PATH\"\nalias ll='ls -la'\n\
             # [clenv: nvm]\nexport NVM_DIR=\"$HOME/.nvm\"\n# [/clenv: nvm]\n\
             alias g='git'\n\
             # [clenv: zoxide]\neval \"$(zoxide init zsh)\"\n# [/clenv: zoxide]\n"
        );
        let segs = parse_segments(zshrc.path());
        let kinds: Vec<_> = segs.iter().map(|s| matches!(s.kind, SegmentKind::Clenv(_))).collect();
        // expect: unmanaged, clenv(nvm), unmanaged, clenv(zoxide)
        assert_eq!(kinds, vec![false, true, false, true]);
    }

    #[test]
    fn parse_segments_header_lines_excluded_from_unmanaged() {
        let zshrc = write_tmp(
            "alias ll='ls -la'\n\
             # clenv> nvm | Fast JS runtime\n\
             # [clenv: nvm]\nexport NVM_DIR=\"$HOME/.nvm\"\n# [/clenv: nvm]\n"
        );
        let segs = parse_segments(zshrc.path());
        // Only 1 unmanaged (the alias line) + 1 clenv block
        let unmanaged: Vec<_> = segs.iter().filter(|s| matches!(s.kind, SegmentKind::Unmanaged)).collect();
        assert_eq!(unmanaged.len(), 1);
        assert!(unmanaged[0].content.contains("alias ll"));
        assert!(!unmanaged[0].content.contains("clenv>"));
    }

    #[test]
    fn write_and_remove_block_round_trip() {
        let zshrc = write_tmp("# existing content\n");
        write_block(zshrc.path(), "test-mod", "echo hello").unwrap();
        assert!(has_block(zshrc.path(), "test-mod"));
        let content = std::fs::read_to_string(zshrc.path()).unwrap();
        assert!(content.contains("# [clenv: test-mod]"));
        assert!(content.contains("echo hello"));
        assert!(!content.contains("# clenv>"), "no metadata headers should be written");
        remove_block(zshrc.path(), "test-mod").unwrap();
        assert!(!has_block(zshrc.path(), "test-mod"));
    }

    #[test]
    fn read_block_strips_old_format_header() {
        let zshrc = write_tmp(
            "# [clenv: foo] — managed by clenv, do not edit manually\necho test\n# [/clenv: foo]\n"
        );
        let content = read_block(zshrc.path(), "foo").unwrap();
        assert_eq!(content, "echo test");
    }

    #[test]
    fn parse_segments_returns_empty_for_missing_file() {
        let result = parse_segments(std::path::Path::new("/tmp/does_not_exist_clenv_test.zshrc"));
        assert!(result.is_empty());
    }

    #[test]
    fn write_block_at_inserts_after_named_block() {
        let zshrc = write_tmp(
            "# [clenv: first]\necho first\n# [/clenv: first]\n"
        );
        write_block_at(zshrc.path(), "second", "echo second", Some("first")).unwrap();
        let content = std::fs::read_to_string(zshrc.path()).unwrap();
        let pos_first = content.find("echo first").unwrap();
        let pos_second = content.find("echo second").unwrap();
        assert!(pos_first < pos_second, "second block should appear after first");
    }
}

fn build_block(name: &str, snippet: &str) -> String {
    format!("# [clenv: {name}]\n{snippet}\n# [/clenv: {name}]\n")
}
