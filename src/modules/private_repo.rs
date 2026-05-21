use std::path::Path;
use std::process::Command;
use anyhow::{Result, anyhow};

/// Sync the private dotfiles repo. Clones if target doesn't exist, pulls if it does.
/// target_dir should be ~/.config/clenv/private/
pub fn sync(repo_url: &str, target_dir: &Path) -> Result<()> {
    if target_dir.exists() {
        // Pull
        let output = Command::new("git")
            .arg("-C")
            .arg(target_dir)
            .arg("pull")
            .arg("--ff-only")
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("git pull failed: {}", stderr));
        }
    } else {
        // Clone
        let output = Command::new("git")
            .arg("clone")
            .arg(repo_url)
            .arg(target_dir)
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("git clone failed: {}", stderr));
        }
    }
    Ok(())
}

/// Returns true if a user_extend file exists in the private repo dir.
pub fn user_extend_exists(private_dir: &Path, user_extend: &str) -> bool {
    private_dir.join(user_extend).exists()
}
