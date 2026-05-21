use super::Module;
use std::collections::{HashMap, HashSet};

/// Topological sort of modules by depends_on.
/// Modules that nothing depends on come first.
/// Returns all modules in a valid load order.
pub fn sort_by_deps(modules: &[Module]) -> Vec<&Module> {
    let name_to_idx: HashMap<&str, usize> = modules
        .iter()
        .enumerate()
        .map(|(i, m)| (m.name.as_str(), i))
        .collect();

    let mut result: Vec<&Module> = Vec::new();
    let mut visited: HashSet<&str> = HashSet::new();

    fn visit<'a>(
        name: &'a str,
        modules: &'a [Module],
        name_to_idx: &HashMap<&str, usize>,
        visited: &mut HashSet<&'a str>,
        result: &mut Vec<&'a Module>,
    ) {
        if visited.contains(name) { return; }
        visited.insert(name);
        if let Some(&idx) = name_to_idx.get(name) {
            let module = &modules[idx];
            for dep in &module.depends_on {
                visit(dep, modules, name_to_idx, visited, result);
            }
            result.push(module);
        }
    }

    // Sort by order first so topo sort produces stable output
    let mut ordered: Vec<&Module> = modules.iter().collect();
    ordered.sort_by_key(|m| m.zshrc.order);

    for m in ordered {
        visit(&m.name, modules, &name_to_idx, &mut visited, &mut result);
    }

    result
}
