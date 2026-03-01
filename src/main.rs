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
mod page;
mod segment;
mod tablespace;
mod vfs;
mod table;
mod catalog;
mod sql;
mod executor;

fn main() {
    println!("Aistore storage engine starting...");
    println!(
        "Loaded modules: buffer, heap, index, tablespace, segment, controlfile, lock, infrastructure, page, vfs"
    );

    println!("\nAistore storage engine startup completed!");
}
