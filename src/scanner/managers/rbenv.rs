use std::path::PathBuf;

/// Parse `rbenv versions` output.
/// Sample:
///   system
/// * 3.2.2 (set by /home/user/.rbenv/version)
///   3.1.4
pub fn parse_output(output: &str) -> Vec<(String, PathBuf)> {
    let rbenv_root = std::env::var("RBENV_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| dirs::home_dir().unwrap_or_default().join(".rbenv"));

    output
        .lines()
        .map(|l| l.trim_start_matches('*').trim())
        .filter(|l| !l.is_empty() && !l.starts_with("system"))
        .filter_map(|line| {
            let version = line.split_whitespace().next()?;
            let path = rbenv_root.join("versions").join(version);
            Some((version.to_string(), path))
        })
        .collect()
}

pub fn discover() -> Vec<(String, PathBuf)> {
    let Ok(output) = std::process::Command::new("rbenv")
        .arg("versions")
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
    fn parses_rbenv_versions() {
        let output = "  system\n* 3.2.2 (set by /home/user/.rbenv/version)\n  3.1.4\n";
        let envs = parse_output(output);
        assert_eq!(envs.len(), 2);
        assert!(envs.iter().any(|(v, _)| v == "3.2.2"));
    }
}
