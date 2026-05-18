use std::path::PathBuf;

/// Parse `nvm list` output.
/// Sample:
///   ->     v20.11.0
///          v18.19.0
///    default -> 20 (-> v20.11.0)
pub fn parse_output(output: &str) -> Vec<(String, PathBuf)> {
    let nvm_dir = dirs::home_dir()
        .unwrap_or_default()
        .join(".nvm")
        .join("versions")
        .join("node");

    output
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            // Find lines containing a version like v20.11.0 but not alias lines
            let version = trimmed
                .split_whitespace()
                .find(|w| w.starts_with('v') && w.chars().nth(1).map(|c| c.is_ascii_digit()).unwrap_or(false))?;
            // Skip lines that are alias definitions (contain "->")
            if trimmed.contains("->") && !trimmed.starts_with("->") && !trimmed.starts_with('*') {
                return None;
            }
            let path = nvm_dir.join(version);
            Some((version.to_string(), path))
        })
        .collect()
}

pub fn discover() -> Vec<(String, PathBuf)> {
    let Ok(output) = std::process::Command::new("bash")
        .args(["-c", "source ~/.nvm/nvm.sh 2>/dev/null && nvm list --no-colors 2>/dev/null"])
        .output()
    else {
        return vec![];
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_output(&stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nvm_list() {
        let output = "->     v20.11.0\n       v18.19.0\n";
        let envs = parse_output(output);
        assert!(envs.iter().any(|(v, _)| v == "v20.11.0"), "should find v20.11.0");
        assert!(envs.iter().any(|(v, _)| v == "v18.19.0"), "should find v18.19.0");
    }
}
