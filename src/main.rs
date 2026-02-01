//! Aistore main program entry

// Use jemalloc as global allocator
#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

// Global type definitions
mod types;

// Import various modules
mod buffer;
mod controlfile;
mod heap;
mod index;
mod infrastructure;
mod lock;
mod segment;
mod tablespace;

fn main() {
    println!("Aistore storage engine starting...");
    println!(
        "Loaded modules: buffer, heap, index, tablespace, segment, controlfile, lock, infrastructure"
    );

    println!("\nAistore storage engine startup completed!");
}
