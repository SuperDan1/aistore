// LRU implementation with hot, cold, and candidate list using doubly linked list

use std::ptr::NonNull;
use std::marker::PhantomData;

/// LRU node structure for the doubly linked list
pub struct LruNode<T> {
    /// Pointer to the previous node
    pub prev: Option<NonNull<LruNode<T>>>,
    /// Pointer to the next node
    pub next: Option<NonNull<LruNode<T>>>,
    /// The actual data stored in the node
    pub data: T,
    /// Flag indicating if the node is in the candidate list
    pub is_candidate: bool,
}

impl<T> LruNode<T> {
    /// Create a new LRU node
    pub fn new(data: T) -> Self {
        LruNode {
            prev: None,
            next: None,
            data,
            is_candidate: false,
        }
    }
}

/// Doubly linked list structure for LRU
pub struct DoublyLinkedList<T> {
    /// Pointer to the head of the list
    pub head: Option<NonNull<LruNode<T>>>,
    /// Pointer to the tail of the list
    pub tail: Option<NonNull<LruNode<T>>>,
    /// Number of nodes in the list
    pub length: usize,
    /// Phantom data for ownership
    pub _marker: PhantomData<Box<LruNode<T>>>,
}

impl<T> DoublyLinkedList<T> {
    /// Create a new empty doubly linked list
    pub fn new() -> Self {
        DoublyLinkedList {
            head: None,
            tail: None,
            length: 0,
            _marker: PhantomData,
        }
    }

    /// Push a node to the front of the list
    pub unsafe fn push_front(&mut self, node: NonNull<LruNode<T>>) {
        // Set the new node's next to current head
        (*node.as_ptr()).next = self.head;
        // Set the new node's prev to None
        (*node.as_ptr()).prev = None;
        
        // Update current head's prev if exists
        if let Some(head) = self.head {
            (*head.as_ptr()).prev = Some(node);
        } else {
            // If list was empty, update tail as well
            self.tail = Some(node);
        }
        
        // Update head to new node
        self.head = Some(node);
        // Increment length
        self.length += 1;
    }

    /// Push a node to the back of the list
    pub unsafe fn push_back(&mut self, node: NonNull<LruNode<T>>) {
        // Set the new node's prev to current tail
        (*node.as_ptr()).prev = self.tail;
        // Set the new node's next to None
        (*node.as_ptr()).next = None;
        
        // Update current tail's next if exists
        if let Some(tail) = self.tail {
            (*tail.as_ptr()).next = Some(node);
        } else {
            // If list was empty, update head as well
            self.head = Some(node);
        }
        
        // Update tail to new node
        self.tail = Some(node);
        // Increment length
        self.length += 1;
    }

    /// Remove a node from the list
    pub unsafe fn remove(&mut self, node: NonNull<LruNode<T>>) {
        let prev = (*node.as_ptr()).prev;
        let next = (*node.as_ptr()).next;
        
        // Update previous node's next if exists
        if let Some(prev_node) = prev {
            (*prev_node.as_ptr()).next = next;
        } else {
            // If removing head, update head to next node
            self.head = next;
        }
        
        // Update next node's prev if exists
        if let Some(next_node) = next {
            (*next_node.as_ptr()).prev = prev;
        } else {
            // If removing tail, update tail to previous node
            self.tail = prev;
        }
        
        // Clear the node's prev and next pointers
        (*node.as_ptr()).prev = None;
        (*node.as_ptr()).next = None;
        
        // Decrement length
        self.length -= 1;
    }

    /// Pop the last node from the list
    pub unsafe fn pop_back(&mut self) -> Option<NonNull<LruNode<T>>> {
        self.tail.map(|tail| {
            self.remove(tail);
            tail
        })
    }

    /// Move a node to the front of the list
    pub unsafe fn move_to_front(&mut self, node: NonNull<LruNode<T>>) {
        self.remove(node);
        self.push_front(node);
    }
}

/// LRU manager with hot, cold, and candidate list
pub struct LruManager<T> {
    /// Hot list: frequently accessed items
    pub hot_list: DoublyLinkedList<T>,
    /// Cold list: infrequently accessed items
    pub cold_list: DoublyLinkedList<T>,
    /// Candidate list: items transitioning between hot and cold
    pub candidate_list: DoublyLinkedList<T>,
    /// Maximum capacity for the hot list
    pub hot_capacity: usize,
    /// Maximum capacity for the cold list
    pub cold_capacity: usize,
    /// Maximum capacity for the candidate list
    pub candidate_capacity: usize,
}

impl<T> LruManager<T> {
    /// Create a new LRU manager with the specified capacities
    pub fn new(hot_capacity: usize, cold_capacity: usize, candidate_capacity: usize) -> Self {
        LruManager {
            hot_list: DoublyLinkedList::new(),
            cold_list: DoublyLinkedList::new(),
            candidate_list: DoublyLinkedList::new(),
            hot_capacity,
            cold_capacity,
            candidate_capacity,
        }
    }

    /// Add an item to the LRU manager
    pub unsafe fn add(&mut self, node: NonNull<LruNode<T>>) {
        // By default, add to the cold list first
        self.cold_list.push_front(node);
        
        // Check if cold list exceeds capacity
        if self.cold_list.length > self.cold_capacity {
            // Evict from cold list if it exceeds capacity
            if let Some(evicted) = self.cold_list.pop_back() {
                // Move evicted item to candidate list
                (*evicted.as_ptr()).is_candidate = true;
                self.candidate_list.push_front(evicted);
                
                // Check if candidate list exceeds capacity
                if self.candidate_list.length > self.candidate_capacity {
                    // Evict from candidate list if it exceeds capacity
                    self.candidate_list.pop_back();
                }
            }
        }
    }

    /// Access an item in the LRU manager
    pub unsafe fn access(&mut self, node: NonNull<LruNode<T>>) {
        // Check if the node is in candidate list
        if (*node.as_ptr()).is_candidate {
            // Remove from candidate list
            self.candidate_list.remove(node);
            (*node.as_ptr()).is_candidate = false;
            
            // Add to hot list
            self.hot_list.push_front(node);
            
            // Check if hot list exceeds capacity
            if self.hot_list.length > self.hot_capacity {
                // Move the least recently used item from hot to cold
                if let Some(lru_hot) = self.hot_list.pop_back() {
                    self.cold_list.push_front(lru_hot);
                    
                    // Check if cold list exceeds capacity
                    if self.cold_list.length > self.cold_capacity {
                        // Evict from cold list if it exceeds capacity
                        if let Some(evicted) = self.cold_list.pop_back() {
                            // Move evicted item to candidate list
                            (*evicted.as_ptr()).is_candidate = true;
                            self.candidate_list.push_front(evicted);
                            
                            // Check if candidate list exceeds capacity
                            if self.candidate_list.length > self.candidate_capacity {
                                // Evict from candidate list if it exceeds capacity
                                self.candidate_list.pop_back();
                            }
                        }
                    }
                }
            }
        } else if self.is_in_list(&self.cold_list, node) {
            // If in cold list, move to hot list
            self.cold_list.remove(node);
            self.hot_list.push_front(node);
            
            // Check if hot list exceeds capacity
            if self.hot_list.length > self.hot_capacity {
                // Move the least recently used item from hot to cold
                if let Some(lru_hot) = self.hot_list.pop_back() {
                    self.cold_list.push_front(lru_hot);
                }
            }
        } else if self.is_in_list(&self.hot_list, node) {
            // If already in hot list, move to front
            self.hot_list.move_to_front(node);
        }
    }

    /// Check if a node is in the given list
    pub unsafe fn is_in_list(&self, list: &DoublyLinkedList<T>, node: NonNull<LruNode<T>>) -> bool {
        let mut current = list.head;
        while let Some(curr_node) = current {
            if curr_node == node {
                return true;
            }
            current = (*curr_node.as_ptr()).next;
        }
        false
    }

    /// Evict the least recently used item from the LRU
    pub unsafe fn evict(&mut self) -> Option<NonNull<LruNode<T>>> {
        // First try to evict from candidate list
        if self.candidate_list.length > 0 {
            return self.candidate_list.pop_back();
        }
        
        // Then try to evict from cold list
        if self.cold_list.length > 0 {
            return self.cold_list.pop_back();
        }
        
        // Finally try to evict from hot list
        self.hot_list.pop_back()
    }
}