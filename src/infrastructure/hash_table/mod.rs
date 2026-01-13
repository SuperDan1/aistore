// Hash table implementation with separate chaining

use std::hash::Hash;
use std::ptr;
use std::alloc;

/// Hash table node structure
struct HashNode<K, V> {
    key: K,
    value: V,
    next: *mut HashNode<K, V>,
}

impl<K, V> HashNode<K, V> {
    /// Create a new hash node
    fn new(key: K, value: V) -> Self {
        HashNode {
            key,
            value,
            next: ptr::null_mut(),
        }
    }
}

/// Hash table structure
pub struct HashTable<K, V> {
    /// Array of buckets, each bucket is a pointer to the first node in the linked list
    buckets: *mut *mut HashNode<K, V>,
    /// Number of buckets in the hash table
    bucket_count: usize,
    /// Number of elements in the hash table
    size: usize,
}

impl<K, V> HashTable<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    /// Create a new hash table with the specified number of buckets
    pub fn new(bucket_count: usize) -> Self {
        // Allocate memory for the buckets array
        let buckets_ptr = unsafe {
            let bucket_size = std::mem::size_of::<*mut HashNode<K, V>>();
            let bucket_align = std::mem::align_of::<*mut HashNode<K, V>>();
            let ptr = alloc::alloc(alloc::Layout::from_size_align_unchecked(
                bucket_size * bucket_count,
                bucket_align,
            )) as *mut *mut HashNode<K, V>;

            // Initialize all buckets to null pointers
            for i in 0..bucket_count {
                *ptr.add(i) = ptr::null_mut();
            }

            ptr
        };

        HashTable {
            buckets: buckets_ptr,
            bucket_count,
            size: 0,
        }
    }

    /// Calculate the bucket index for a given key
    fn bucket_index(&self, key: &K) -> usize {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hasher;

        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let hash = hasher.finish();
        (hash as usize) % self.bucket_count
    }

    /// Insert a key-value pair into the hash table
    pub fn insert(&mut self, key: K, value: V) {
        // Calculate the bucket index
        let index = self.bucket_index(&key);

        unsafe {
            // Check if the key already exists
            let mut current = *self.buckets.add(index);
            while !current.is_null() {
                if (*current).key == key {
                    // Update existing value
                    (*current).value = value;
                    return;
                }
                current = (*current).next;
            }

            // Create a new hash node
            let node_size = std::mem::size_of::<HashNode<K, V>>();
            let node_align = std::mem::align_of::<HashNode<K, V>>();
            let new_node_ptr = alloc::alloc(alloc::Layout::from_size_align_unchecked(
                node_size,
                node_align,
            )) as *mut HashNode<K, V>;

            // Initialize the new node
            *new_node_ptr = HashNode::new(key, value);

            // Insert the new node at the beginning of the linked list
            (*new_node_ptr).next = *self.buckets.add(index);
            *self.buckets.add(index) = new_node_ptr;

            // Increment the size
            self.size += 1;
        }
    }

    /// Get the value associated with a key
    pub fn get(&self, key: &K) -> Option<V> {
        // Calculate the bucket index
        let index = self.bucket_index(key);

        unsafe {
            // Traverse the linked list at the calculated index
            let mut current = *self.buckets.add(index);
            while !current.is_null() {
                if &(*current).key == key {
                    // Return a clone of the value
                    return Some(((*current).value).clone());
                }
                current = (*current).next;
            }
        }

        // Key not found
        None
    }

    /// Remove a key-value pair from the hash table
    pub fn remove(&mut self, key: &K) -> Option<V> {
        // Calculate the bucket index
        let index = self.bucket_index(key);

        unsafe {
            let bucket_ptr = self.buckets.add(index);
            let mut current = *bucket_ptr;
            let mut prev: *mut HashNode<K, V> = ptr::null_mut();

            // Traverse the linked list to find the key
            while !current.is_null() {
                if &(*current).key == key {
                    // Key found
                    let value = (*current).value.clone();

                    // Remove the node from the linked list
                    if prev.is_null() {
                        // Node is the head of the list
                        *bucket_ptr = (*current).next;
                    } else {
                        (*prev).next = (*current).next;
                    }

                    // Deallocate the node
                    let node_size = std::mem::size_of::<HashNode<K, V>>();
                    let node_align = std::mem::align_of::<HashNode<K, V>>();
                    alloc::dealloc(
                        current as *mut u8,
                        alloc::Layout::from_size_align_unchecked(node_size, node_align),
                    );

                    // Decrement the size
                    self.size -= 1;

                    return Some(value);
                }

                prev = current;
                current = (*current).next;
            }
        }

        // Key not found
        None
    }

    /// Get the number of elements in the hash table
    pub fn size(&self) -> usize {
        self.size
    }

    /// Check if the hash table is empty
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }
}

/// Implement Drop trait to properly deallocate memory
impl<K, V> Drop for HashTable<K, V> {
    fn drop(&mut self) {
        unsafe {
            // Iterate through all buckets
            for i in 0..self.bucket_count {
                let mut current = *self.buckets.add(i);
                
                // Traverse and deallocate each node in the linked list
                while !current.is_null() {
                    let next = (*current).next;
                    
                    // Deallocate the current node
                    let node_size = std::mem::size_of::<HashNode<K, V>>();
                    let node_align = std::mem::align_of::<HashNode<K, V>>();
                    alloc::dealloc(
                        current as *mut u8,
                        alloc::Layout::from_size_align_unchecked(node_size, node_align),
                    );
                    
                    current = next;
                }
            }
            
            // Deallocate the buckets array
            let bucket_size = std::mem::size_of::<*mut HashNode<K, V>>();
            let bucket_align = std::mem::align_of::<*mut HashNode<K, V>>();
            alloc::dealloc(
                self.buckets as *mut u8,
                alloc::Layout::from_size_align_unchecked(
                    bucket_size * self.bucket_count,
                    bucket_align,
                ),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    include!("tests.rs");
}
