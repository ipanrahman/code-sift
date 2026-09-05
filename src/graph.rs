//! Reference index for tracking code relationships.

use crate::types::{FileId, Relationship, SymbolId};
use hashbrown::HashMap;

/// Reference index tracking symbol relationships.
#[derive(Clone)]
pub struct ReferenceIndex {
    /// References FROM a symbol: symbol_id -> [(target_id, relationship)]
    outgoing: HashMap<SymbolId, Vec<(SymbolId, Relationship)>>,
    /// References TO a symbol: symbol_id -> [(source_id, relationship)]
    incoming: HashMap<SymbolId, Vec<(SymbolId, Relationship)>>,
    /// Call graph: function -> functions it calls
    call_graph: HashMap<SymbolId, Vec<SymbolId>>,
    /// Reverse call graph: function -> functions that call it
    callers: HashMap<SymbolId, Vec<SymbolId>>,
}

impl ReferenceIndex {
    pub fn new() -> Self {
        Self {
            outgoing: HashMap::new(),
            incoming: HashMap::new(),
            call_graph: HashMap::new(),
            callers: HashMap::new(),
        }
    }

    /// Add a reference from one symbol to another.
    pub fn add_reference(&mut self, from: SymbolId, to: SymbolId, rel: Relationship) {
        self.outgoing.entry(from).or_default().push((to, rel));
        self.incoming.entry(to).or_default().push((from, rel));

        // Track call relationships separately for graph traversal
        if matches!(rel, Relationship::Calls) {
            self.call_graph.entry(from).or_default().push(to);
            self.callers.entry(to).or_default().push(from);
        }
    }

    /// Get symbols called by this symbol.
    pub fn get_callees(&self, id: SymbolId) -> &[SymbolId] {
        self.call_graph.get(&id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get symbols that call this symbol.
    pub fn get_callers(&self, id: SymbolId) -> &[SymbolId] {
        self.callers.get(&id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get all outgoing references.
    pub fn get_outgoing(&self, id: SymbolId) -> &[(SymbolId, Relationship)] {
        self.outgoing.get(&id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get all incoming references.
    pub fn get_incoming(&self, id: SymbolId) -> &[(SymbolId, Relationship)] {
        self.incoming.get(&id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Traverse call graph up to a bounded depth.
    pub fn traverse_callers(&self, id: SymbolId, max_depth: usize) -> Vec<(SymbolId, usize)> {
        let mut results = Vec::new();
        let mut visited = HashMap::new();
        self.traverse_callers_inner(id, 0, max_depth, &mut results, &mut visited);
        results
    }

    fn traverse_callers_inner(
        &self,
        id: SymbolId,
        depth: usize,
        max_depth: usize,
        results: &mut Vec<(SymbolId, usize)>,
        visited: &mut HashMap<SymbolId, usize>,
    ) {
        if depth >= max_depth {
            return;
        }

        for caller in self.get_callers(id) {
            let prev_depth = visited.get(caller).copied().unwrap_or(usize::MAX);
            if depth + 1 < prev_depth {
                visited.insert(*caller, depth + 1);
                results.push((*caller, depth + 1));
                self.traverse_callers_inner(*caller, depth + 1, max_depth, results, visited);
            }
        }
    }

    /// Traverse call graph down to a bounded depth.
    pub fn traverse_callees(&self, id: SymbolId, max_depth: usize) -> Vec<(SymbolId, usize)> {
        let mut results = Vec::new();
        let mut visited = HashMap::new();
        self.traverse_callees_inner(id, 0, max_depth, &mut results, &mut visited);
        results
    }

    fn traverse_callees_inner(
        &self,
        id: SymbolId,
        depth: usize,
        max_depth: usize,
        results: &mut Vec<(SymbolId, usize)>,
        visited: &mut HashMap<SymbolId, usize>,
    ) {
        if depth >= max_depth {
            return;
        }

        for callee in self.get_callees(id) {
            let prev_depth = visited.get(callee).copied().unwrap_or(usize::MAX);
            if depth + 1 < prev_depth {
                visited.insert(*callee, depth + 1);
                results.push((*callee, depth + 1));
                self.traverse_callees_inner(*callee, depth + 1, max_depth, results, visited);
            }
        }
    }
}

impl Default for ReferenceIndex {
    fn default() -> Self {
        Self::new()
    }
}
