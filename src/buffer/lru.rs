// LRU implementation with hot, cold, and candidate list using standard library LinkedList

use std::collections::LinkedList;

/// Node type indicating which list the node belongs to
#[derive(Clone, PartialEq, Debug)]
pub enum NodeType {
    /// Node is in the hot list
    Hot,
    /// Node is in the cold list
    Cold,
    /// Node is in the free list
    Free,
}

/// LRU node structure for the linked list
#[derive(Clone)]
pub struct Node<T> {
    /// The actual data stored in the node
    pub data: T,
    /// The type of the node, indicating which list it belongs to
    pub node_type: NodeType,
}

impl<T> Node<T> {
    /// Create a new LRU node with the specified type
    pub fn new(data: T, node_type: NodeType) -> Self {
        Node { data, node_type }
    }
}

/// LRU manager with hot, cold, and candidate list
pub struct LruManager<T> {
    /// Hot list: frequently accessed items
    pub hot_list: LinkedList<Node<T>>,
    /// Cold list: infrequently accessed items
    pub cold_list: LinkedList<Node<T>>,
    /// Free list: items that can be evicted
    pub free_list: LinkedList<Node<T>>,
    /// Maximum capacity for the hot list
    pub hot_capacity: usize,
    /// Maximum capacity for the cold list
    pub cold_capacity: usize,
    /// Maximum capacity for the free list
    pub free_capacity: usize,
}

impl<T> LruManager<T>
where
    T: Clone + PartialEq,
{
    /// Create a new LRU manager with the specified capacities
    pub fn new(hot_capacity: usize, cold_capacity: usize, free_capacity: usize) -> Self {
        LruManager {
            hot_list: LinkedList::new(),
            cold_list: LinkedList::new(),
            free_list: LinkedList::new(),
            hot_capacity,
            cold_capacity,
            free_capacity,
        }
    }

    /// Add an item to the LRU manager
    pub fn add(&mut self, data: T) {
        // By default, add to the cold list first
        self.cold_list.push_front(Node::new(data, NodeType::Cold));

        // Check if cold list exceeds capacity
        if self.cold_list.len() > self.cold_capacity {
            // Evict from cold list if it exceeds capacity
            if let Some(evicted) = self.cold_list.pop_back() {
                // Move evicted item to free list
                let mut free_node = evicted.clone();
                free_node.node_type = NodeType::Free;
                self.free_list.push_front(free_node);

                // Check if free list exceeds capacity
                if self.free_list.len() > self.free_capacity {
                    // Evict from free list if it exceeds capacity
                    self.free_list.pop_back();
                }
            }
        }
    }

    /// Find and access an item in the LRU manager
    pub fn access(&mut self, data: &T) {
        // Check if the node is in free list
        if let Some(index) = self.free_list.iter().position(|node| &node.data == data) {
            // Create a new list without the found node
            let mut new_list = LinkedList::new();
            let mut removed_node = None;

            // Iterate through the original list and copy nodes to the new list except the one to remove
            for (i, node) in self.free_list.iter().enumerate() {
                if i == index {
                    removed_node = Some(node.clone());
                } else {
                    new_list.push_back(node.clone());
                }
            }

            // Replace the original list with the new one
            self.free_list = new_list;

            if let Some(node) = removed_node {
                // Add to hot list
                let mut hot_node = node.clone();
                hot_node.node_type = NodeType::Hot;
                self.hot_list.push_front(hot_node);

                // Check if hot list exceeds capacity
                if self.hot_list.len() > self.hot_capacity {
                    // Move the least recently used item from hot to cold
                    if let Some(lru_hot) = self.hot_list.pop_back() {
                        let mut cold_node = lru_hot.clone();
                        cold_node.node_type = NodeType::Cold;
                        self.cold_list.push_front(cold_node);

                        // Check if cold list exceeds capacity
                        if self.cold_list.len() > self.cold_capacity {
                            // Evict from cold list if it exceeds capacity
                            if let Some(evicted) = self.cold_list.pop_back() {
                                // Move evicted item to free list
                                let mut free_node = evicted.clone();
                                free_node.node_type = NodeType::Free;
                                self.free_list.push_front(free_node);

                                // Check if free list exceeds capacity
                                if self.free_list.len() > self.free_capacity {
                                    // Evict from free list if it exceeds capacity
                                    self.free_list.pop_back();
                                }
                            }
                        }
                    }
                }
            }
        }
        // Check if the node is in cold list
        else if let Some(index) = self.cold_list.iter().position(|node| &node.data == data) {
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
                let mut hot_node = node.clone();
                hot_node.node_type = NodeType::Hot;
                self.hot_list.push_front(hot_node);

                // Check if hot list exceeds capacity
                if self.hot_list.len() > self.hot_capacity {
                    // Move the least recently used item from hot to cold
                    if let Some(lru_hot) = self.hot_list.pop_back() {
                        let mut cold_node = lru_hot.clone();
                        cold_node.node_type = NodeType::Cold;
                        self.cold_list.push_front(cold_node);
                    }
                }
            }
        }
        // Check if the node is in hot list
        else if let Some(index) = self.hot_list.iter().position(|node| &node.data == data) {
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
                let mut hot_node = node.clone();
                hot_node.node_type = NodeType::Hot;
                self.hot_list.push_front(hot_node);
            }
        }
    }

    /// Evict the least recently used item from the LRU
    pub fn evict(&mut self) -> Option<Node<T>> {
        // First try to evict from free list
        if !self.free_list.is_empty() {
            return self.free_list.pop_back();
        }

        // Then try to evict from cold list
        if !self.cold_list.is_empty() {
            return self.cold_list.pop_back();
        }

        // Finally try to evict from hot list
        self.hot_list.pop_back()
    }
}
