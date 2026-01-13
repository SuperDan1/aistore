// LRU implementation with hot, cold, and candidate list using standard library LinkedList

use std::collections::LinkedList;

/// LRU node structure for the linked list
#[derive(Clone)]
pub struct Node<T> {
    /// The actual data stored in the node
    pub data: T,
}

impl<T> Node<T> {
    /// Create a new LRU node
    pub fn new(data: T) -> Self {
        Node {
            data,
        }
    }
}

/// LRU manager with hot, cold, and candidate list
pub struct LruManager<T> {
    /// Hot list: frequently accessed items
    pub hot_list: LinkedList<Node<T>>,
    /// Cold list: infrequently accessed items
    pub cold_list: LinkedList<Node<T>>,
    /// Candidate list: items transitioning between hot and cold
    pub candidate_list: LinkedList<Node<T>>,
    /// Maximum capacity for the hot list
    pub hot_capacity: usize,
    /// Maximum capacity for the cold list
    pub cold_capacity: usize,
    /// Maximum capacity for the candidate list
    pub candidate_capacity: usize,
}

impl<T> LruManager<T>
where
    T: Clone + PartialEq,
{
    /// Create a new LRU manager with the specified capacities
    pub fn new(hot_capacity: usize, cold_capacity: usize, candidate_capacity: usize) -> Self {
        LruManager {
            hot_list: LinkedList::new(),
            cold_list: LinkedList::new(),
            candidate_list: LinkedList::new(),
            hot_capacity,
            cold_capacity,
            candidate_capacity,
        }
    }

    /// Add an item to the LRU manager
    pub fn add(&mut self, data: T) {
        // By default, add to the cold list first
        self.cold_list.push_front(Node::new(data));
        
        // Check if cold list exceeds capacity
        if self.cold_list.len() > self.cold_capacity {
            // Evict from cold list if it exceeds capacity
            if let Some(evicted) = self.cold_list.pop_back() {
                // Move evicted item to candidate list
                self.candidate_list.push_front(evicted);
                
                // Check if candidate list exceeds capacity
                if self.candidate_list.len() > self.candidate_capacity {
                    // Evict from candidate list if it exceeds capacity
                    self.candidate_list.pop_back();
                }
            }
        }
    }

    /// Find and access an item in the LRU manager
    pub fn access(&mut self, data: &T) {
        // Check if the node is in candidate list
        if let Some(index) = self.candidate_list
            .iter()
            .position(|node| &node.data == data) {    
            // Create a new list without the found node
            let mut new_list = LinkedList::new();
            let mut removed_node = None;
            
            // Iterate through the original list and copy nodes to the new list except the one to remove
            for (i, node) in self.candidate_list.iter().enumerate() {
                if i == index {
                    removed_node = Some(node.clone());
                } else {
                    new_list.push_back(node.clone());
                }
            }
            
            // Replace the original list with the new one
            self.candidate_list = new_list;
            
            if let Some(node) = removed_node {
                // Add to hot list
                self.hot_list.push_front(node);
                
                // Check if hot list exceeds capacity
                if self.hot_list.len() > self.hot_capacity {
                    // Move the least recently used item from hot to cold
                    if let Some(lru_hot) = self.hot_list.pop_back() {
                        self.cold_list.push_front(lru_hot);
                        
                        // Check if cold list exceeds capacity
                        if self.cold_list.len() > self.cold_capacity {
                            // Evict from cold list if it exceeds capacity
                            if let Some(evicted) = self.cold_list.pop_back() {
                                // Move evicted item to candidate list
                                self.candidate_list.push_front(evicted);
                                
                                // Check if candidate list exceeds capacity
                                if self.candidate_list.len() > self.candidate_capacity {
                                    // Evict from candidate list if it exceeds capacity
                                    self.candidate_list.pop_back();
                                }
                            }
                        }
                    }
                }
            }
        }
        // Check if the node is in cold list
        else if let Some(index) = self.cold_list
            .iter()
            .position(|node| &node.data == data) {    
            // Create a new list without the found node
            let mut new_list = LinkedList::new();
            let mut removed_node = None;
            
            // Iterate through the original list and copy nodes to the new list except the one to remove
            for (i, node) in self.cold_list.iter().enumerate() {
                if i == index {
                    removed_node = Some(node.clone());
                } else {
                    new_list.push_back(node.clone());
                }
            }
            
            // Replace the original list with the new one
            self.cold_list = new_list;
            
            if let Some(node) = removed_node {
                // Add to hot list
                self.hot_list.push_front(node);
                
                // Check if hot list exceeds capacity
                if self.hot_list.len() > self.hot_capacity {
                    // Move the least recently used item from hot to cold
                    if let Some(lru_hot) = self.hot_list.pop_back() {
                        self.cold_list.push_front(lru_hot);
                    }
                }
            }
        }
        // Check if the node is in hot list
        else if let Some(index) = self.hot_list
            .iter()
            .position(|node| &node.data == data) {    
            // Create a new list without the found node
            let mut new_list = LinkedList::new();
            let mut removed_node = None;
            
            // Iterate through the original list and copy nodes to the new list except the one to remove
            for (i, node) in self.hot_list.iter().enumerate() {
                if i == index {
                    removed_node = Some(node.clone());
                } else {
                    new_list.push_back(node.clone());
                }
            }
            
            // Replace the original list with the new one
            self.hot_list = new_list;
            
            if let Some(node) = removed_node {
                // Add to front of hot list
                self.hot_list.push_front(node);
            }
        }
    }

    /// Evict the least recently used item from the LRU
    pub fn evict(&mut self) -> Option<Node<T>> {
        // First try to evict from candidate list
        if !self.candidate_list.is_empty() {
            return self.candidate_list.pop_back();
        }
        
        // Then try to evict from cold list
        if !self.cold_list.is_empty() {
            return self.cold_list.pop_back();
        }
        
        // Finally try to evict from hot list
        self.hot_list.pop_back()
    }
}