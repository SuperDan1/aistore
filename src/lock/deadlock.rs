//! Deadlock detection

use super::TransactionId;
use std::collections::HashSet;

/// Wait-for graph edge
#[derive(Debug, Clone)]
struct WaitEdge {
    from: TransactionId,
    to: TransactionId,
}

/// Deadlock detector using wait-for graph
pub struct DeadlockDetector {
    edges: Vec<WaitEdge>,
}

impl DeadlockDetector {
    pub fn new() -> Self {
        Self { edges: Vec::new() }
    }

    /// Add a wait edge: tx1 is waiting for tx2
    pub fn add_edge(&mut self, from: TransactionId, to: TransactionId) {
        if !self.edges.iter().any(|e| e.from == from && e.to == to) {
            self.edges.push(WaitEdge { from, to });
        }
    }

    /// Remove all edges from a transaction
    pub fn remove_edges_from(&mut self, tx_id: TransactionId) {
        self.edges.retain(|e| e.from != tx_id && e.to != tx_id);
    }

    /// Detect deadlock using DFS
    pub fn detect(&self, start_tx: TransactionId) -> Option<TransactionId> {
        let mut visited: HashSet<TransactionId> = HashSet::new();
        let mut path: Vec<TransactionId> = Vec::new();

        if self.detect_cycle(start_tx, &mut visited, &mut path) {
            Some(*path.last().unwrap())
        } else {
            None
        }
    }

    fn detect_cycle(
        &self,
        tx: TransactionId,
        visited: &mut HashSet<TransactionId>,
        path: &mut Vec<TransactionId>,
    ) -> bool {
        visited.insert(tx);
        path.push(tx);

        let waiting_for: Vec<TransactionId> = self
            .edges
            .iter()
            .filter(|e| e.from == tx)
            .map(|e| e.to)
            .collect();

        for next_tx in waiting_for {
            if !visited.contains(&next_tx) {
                if self.detect_cycle(next_tx, visited, path) {
                    return true;
                }
            } else if path.contains(&next_tx) {
                return true;
            }
        }

        path.pop();
        false
    }

    pub fn get_deadlock_txs(&self) -> Vec<TransactionId> {
        let mut txs: HashSet<TransactionId> = HashSet::new();
        for edge in &self.edges {
            txs.insert(edge.from);
            txs.insert(edge.to);
        }
        txs.into_iter().collect()
    }
}

impl Default for DeadlockDetector {
    fn default() -> Self {
        Self::new()
    }
}
