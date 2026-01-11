//! Aistore main program entry

// Use jemalloc as global allocator
#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

// Global type definitions
mod types;

// Import various modules
mod buffer;
mod heap;
mod index;
mod tablespace;
mod segment;
mod controlfile;
mod lock;
mod infrastructure;

fn main() {
    println!("Aistore storage engine starting...");
    println!("Loaded modules: buffer, heap, index, tablespace, segment, controlfile, lock, infrastructure");
    println!("Aistore storage engine startup completed!");
}
