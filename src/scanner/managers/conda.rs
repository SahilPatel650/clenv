use std::path::PathBuf;

/// Parse `conda env list` output into (name, path) pairs.
/// Sample output:
///   # conda environments:
///   base                  *  /opt/miniconda3
///   myenv                    /opt/miniconda3/envs/myenv
pub fn parse_output(output: &str) -> Vec<(String, PathBuf)> {
    output
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            match parts.as_slice() {
                [name, path] => Some((name.to_string(), PathBuf::from(path))),
                [name, "*", path] => Some((name.to_string(), PathBuf::from(path))),
                _ => None,
            }
        })
        .collect()
}

pub fn discover() -> Vec<(String, PathBuf)> {
    let Ok(output) = std::process::Command::new("conda")
        .args(["env", "list"])
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
    fn parses_conda_env_list() {
        let output = "# conda environments:\n\
                      base                  *  /opt/miniconda3\n\
                      myenv                    /opt/miniconda3/envs/myenv\n";
        let envs = parse_output(output);
        assert_eq!(envs.len(), 2);
        assert_eq!(envs[0].0, "base");
        assert_eq!(envs[0].1, PathBuf::from("/opt/miniconda3"));
        assert_eq!(envs[1].0, "myenv");
    }
}
