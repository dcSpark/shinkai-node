use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ColumnDependencyManager {
    // Column -> Columns it depends on
    pub dependencies: HashMap<usize, HashSet<usize>>,
    // Column -> Columns that depend on it
    pub reverse_dependencies: HashMap<usize, HashSet<usize>>,
}

impl ColumnDependencyManager {
    pub fn add_dependency(&mut self, from: usize, to: usize) {
        self.dependencies.entry(from).or_default().insert(to);
        self.reverse_dependencies.entry(to).or_default().insert(from);
    }

    pub fn remove_dependency(&mut self, from: usize, to: usize) {
        if let Some(deps) = self.dependencies.get_mut(&from) {
            deps.remove(&to);
        }
        if let Some(rev_deps) = self.reverse_dependencies.get_mut(&to) {
            rev_deps.remove(&from);
        }
    }

    pub fn get_dependents(&self, column: usize) -> HashSet<usize> {
        self.dependencies.get(&column).cloned().unwrap_or_default()
    }

    pub fn get_reverse_dependents(&self, column: usize) -> HashSet<usize> {
        self.reverse_dependencies.get(&column).cloned().unwrap_or_default()
    }
}