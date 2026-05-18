use std::path::PathBuf;

/// Parse `sdk list java` output for installed versions.
/// Lines containing "installed" or "local only":
///   | Temurin    |     | 21.0.2       | tem     | installed  | 21.0.2-tem
pub fn parse_output(output: &str) -> Vec<(String, PathBuf)> {
    output
        .lines()
        .filter(|l| l.contains("installed") || l.contains("local only"))
        .filter_map(|line| {
            let parts: Vec<&str> = line.split('|').collect();
            let identifier = parts.iter().rev().find(|s| !s.trim().is_empty())?.trim();
            if identifier.is_empty() {
                return None;
            }
            let path = dirs::home_dir()
                .unwrap_or_default()
                .join(".sdkman")
                .join("candidates")
                .join("java")
                .join(identifier);
            Some((identifier.to_string(), path))
        })
        .collect()
}

pub fn discover() -> Vec<(String, PathBuf)> {
    let Ok(output) = std::process::Command::new("bash")
        .args(["-c", "source ~/.sdkman/bin/sdkman-init.sh 2>/dev/null && sdk list java 2>/dev/null"])
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
    fn parses_sdkman_list() {
        let output = "| Temurin    |     | 21.0.2       | tem     | installed  | 21.0.2-tem      |\n\
                      | Temurin    |     | 17.0.9       | tem     |            | 17.0.9-tem      |\n";
        let envs = parse_output(output);
        assert_eq!(envs.len(), 1);
        assert_eq!(envs[0].0, "21.0.2-tem");
    }
}
