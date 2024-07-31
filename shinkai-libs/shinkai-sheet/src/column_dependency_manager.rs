use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::sheet::ColumnIndex;

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

    pub fn remove_column(&mut self, col: ColumnIndex) {
        // Remove all dependencies where the column is a key
        if let Some(deps) = self.dependencies.remove(&col) {
            for dep in deps {
                if let Some(rev_deps) = self.reverse_dependencies.get_mut(&dep) {
                    rev_deps.remove(&col);
                }
            }
        }

        // Remove all reverse dependencies where the column is a value
        if let Some(rev_deps) = self.reverse_dependencies.remove(&col) {
            for rev_dep in rev_deps {
                if let Some(deps) = self.dependencies.get_mut(&rev_dep) {
                    deps.remove(&col);
                }
            }
        }
    }

    pub fn update_dependencies(&mut self, col: ColumnIndex, dependencies: HashSet<ColumnIndex>) {
        // Remove existing dependencies for the column
        self.dependencies.remove(&col);
        
        // Add new dependencies
        for &dep in &dependencies {
            self.add_dependency(col, dep);
        }
    }

    pub fn get_dependents(&self, column: usize) -> HashSet<usize> {
        self.dependencies.get(&column).cloned().unwrap_or_default()
    }

    pub fn get_reverse_dependents(&self, column: usize) -> HashSet<usize> {
        self.reverse_dependencies.get(&column).cloned().unwrap_or_default()
    }
}