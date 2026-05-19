use crate::env::EnvSource;
use std::path::PathBuf;

// mamba and micromamba use the same output format as `conda env list`
pub fn discover_tagged() -> Vec<(String, PathBuf, EnvSource)> {
    let mut results: Vec<(String, PathBuf, EnvSource)> = Vec::new();
    for (cmd, source) in &[("mamba", EnvSource::Mamba), ("micromamba", EnvSource::Micromamba)] {
        if let Ok(output) = std::process::Command::new(cmd)
            .args(["env", "list"])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for (name, path) in super::conda::parse_output(&stdout) {
                    results.push((name, path, *source));
                }
            }
        }
    }
    results
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_micromamba_style_output() {
        // micromamba adds a header row and separator that the conda parser should skip
        let output = "\
  Name           Active  Path\n\
─────────────────────────────────────────────────────────────\n\
  base           *       /Users/user/micromamba\n\
  myenv                  /Users/user/micromamba/envs/myenv\n";
        let envs = super::super::conda::parse_output(output);
        assert_eq!(envs.len(), 2);
        assert_eq!(envs[0].0, "base");
        assert_eq!(envs[1].0, "myenv");
    }
}
