pub mod conda;
pub mod mamba;
pub mod nvm;
pub mod pyenv;
pub mod rbenv;
pub mod sdkman;

use crate::env::{EnvKind, Environment};
use std::path::PathBuf;

fn pairs_to_envs(pairs: Vec<(String, PathBuf)>, kind: EnvKind) -> Vec<Environment> {
    pairs
        .into_iter()
        .map(|(name, path)| {
            let mut env = Environment::new(kind.clone(), path);
            env.name = name;
            env
        })
        .collect()
}

/// Collect all manager-discovered environments.
pub fn discover_all() -> Vec<Environment> {
    let mut results: Vec<Environment> = vec![];

    results.extend(pairs_to_envs(conda::discover(), EnvKind::Conda));
    results.extend(pairs_to_envs(mamba::discover(), EnvKind::Conda));
    results.extend(pairs_to_envs(pyenv::discover(), EnvKind::Python));
    results.extend(pairs_to_envs(rbenv::discover(), EnvKind::Ruby));
    results.extend(pairs_to_envs(sdkman::discover(), EnvKind::Java));
    results.extend(pairs_to_envs(nvm::discover(), EnvKind::Node));

    results
}
