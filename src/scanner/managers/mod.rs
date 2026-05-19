pub mod conda;
pub mod mamba;
pub mod nvm;
pub mod pyenv;
pub mod rbenv;
pub mod sdkman;

use crate::env::{EnvKind, EnvSource, Environment};
use std::path::PathBuf;

fn pairs_to_envs(pairs: Vec<(String, PathBuf)>, kind: EnvKind, source: EnvSource) -> Vec<Environment> {
    pairs
        .into_iter()
        .map(|(name, path)| {
            let mut env = Environment::new(kind.clone(), path);
            env.name = name;
            env.source = source;
            env
        })
        .collect()
}

fn tagged_to_envs(tagged: Vec<(String, PathBuf, EnvSource)>, kind: EnvKind) -> Vec<Environment> {
    tagged
        .into_iter()
        .map(|(name, path, source)| {
            let mut env = Environment::new(kind.clone(), path);
            env.name = name;
            env.source = source;
            env
        })
        .collect()
}

/// Collect all manager-discovered environments.
pub fn discover_all() -> Vec<Environment> {
    let mut results: Vec<Environment> = vec![];

    results.extend(pairs_to_envs(conda::discover(), EnvKind::Conda, EnvSource::Conda));
    results.extend(tagged_to_envs(mamba::discover_tagged(), EnvKind::Conda));
    results.extend(pairs_to_envs(pyenv::discover(), EnvKind::Python, EnvSource::Pyenv));
    results.extend(pairs_to_envs(rbenv::discover(), EnvKind::Ruby, EnvSource::Rbenv));
    results.extend(pairs_to_envs(sdkman::discover(), EnvKind::Java, EnvSource::Sdkman));
    results.extend(pairs_to_envs(nvm::discover(), EnvKind::Node, EnvSource::Nvm));

    results
}
