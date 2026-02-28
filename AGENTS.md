# AISTORE KNOWLEDGE BASE

**Generated:** 2026-02-08
**Project:** High-performance storage engine (Rust 2024)

## OVERVIEW
Aistore is a Rust storage engine with segment-page storage, buffer pool caching, and multi-hash algorithm support. Optimized for performance with jemalloc and parking_lot.

## STRUCTURE
```
aistore/
├── src/
│   ├── infrastructure/    # Hash algos, hash tables, lwlock (PRIMITIVES)
│   ├── buffer/            # LRU buffer pool (CACHING)
│   ├── vfs/               # Virtual filesystem (ABSTRACTION)
│   ├── page/              # Page structure (STORAGE UNIT)
│   ├── segment/           # 64MB segments (STORAGE LAYOUT)
│   ├── tablespace/        # Tablespace management (ORG)
│   ├── heap/              # Heap file organization
│   ├── index/             # B-tree indexes
│   ├── table/             # Table metadata, columns (DATA MODEL)
│   ├── catalog/           # System catalog, table persistence
│   ├── lock/              # Lock management
│   ├── controlfile/       # Metadata persistence
│   ├── binlog/            # Binary logging
│   ├── redolog/           # Write-ahead log
│   └── systable/         # System catalogs
├── Cargo.toml
└── AGENTS.md (this file)
```
```
aistore/
├── src/
│   ├── infrastructure/    # Hash algos, hash tables, lwlock (PRIMITIVES)
│   ├── buffer/            # LRU buffer pool (CACHING)
│   ├── vfs/               # Virtual filesystem (ABSTRACTION)
│   ├── page/              # Page structure (STORAGE UNIT)
│   ├── segment/           # 64MB segments (STORAGE LAYOUT)
│   ├── tablespace/        # Tablespace management (ORG)
│   ├── heap/              # Heap file organization
│   ├── index/             # B-tree indexes
│   ├── lock/              # Lock management
│   ├── controlfile/       # Metadata persistence
│   ├── binlog/            # Binary logging
│   ├── redolog/           # Write-ahead log
│   └── systable/         # System catalogs
├── Cargo.toml
└── AGENTS.md (this file)
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Buffer pool | `buffer/` | LRU-K, atomic pin counting, dirty tracking |
| Hashing | `infrastructure/hash/` | FNV-1a, XXH64, CRC32, CityHash benchmarks |
| VFS abstraction | `vfs/` | Trait-based, posix operations |
| Storage layout | `tablespace/`, `segment/` | InnoDB-style, free extent lists |
| Concurrency | `infrastructure/lwlock/` | Custom lightweight locks |

## CONVENTIONS (Deviations from Standard Rust)

### Imports
```rust
use std::sync::Arc;
use parking_lot::RwLock;
use crate::types::BlockId;        // Always crate::
use crate::buffer::BufferMgr;     // Never super::
```

### Naming
| Type | Pattern | Example |
|------|---------|---------|
| Constants | `SCREAMING_SNAKE_CASE` | `BLOCK_SIZE`, `CACHELINE_SIZE` |
| Traits | `PascalCase` + `Interface` | `RwLockInterface<T>` |
| Wrappers | `<Type>Wrapper` | `StdRwLockWrapper<T>` |
| Booleans | `is_`, `has_`, `can_` | `is_dirty()`, `has_pin()` |

### Error Handling
```rust
// Custom Result alias per module
pub type VfsResult<T> = Result<T, VfsError>;

// From conversions required
impl From<std::io::Error> for VfsError { ... }

// NEVER .unwrap() - always ?
```

### Atomic State (Buffer Pool Pattern)
```rust
// 64-bit layout: DIRTY_BIT at bit 0, PIN_COUNT at bits 8-63
const DIRTY_BIT: u64 = 1 << 0;
const PIN_COUNT_SHIFT: u8 = 8;
```

### Memory & Concurrency
- **Global allocator**: jemalloc via `#[global_allocator]`
- **Cache line alignment**: `#[repr(align(64))]` on x86_64
- **Prefer parking_lot** over std::sync

### Non-Standard Patterns (Deviations)
- **Rust 2024 Edition**: `edition = "2024"` in Cargo.toml (bleeding-edge, not 2021)
- **Minimal CI**: No clippy, fmt check, or caching in `.github/workflows/rust.yml`
- **Global allocator**: jemalloc via `#[global_allocator]`
- **Cache line alignment**: `#[repr(align(64))]` on x86_64
- **Prefer parking_lot** over std::sync

## ANTI-PATTERNS (THIS PROJECT)

### CRITICAL - Never Do
- Never use `.unwrap()` in production code
- Never modify buffer state without atomic operations
- Never skip checksum verification on disk reads
- Never mix module-specific `Result` types with standard `Result`

### WARNING
- Never bypass LRU for hot paths
- Never leak raw pointers from extent allocation

## CODE QUALITY (Pre-Commit)

```bash
cargo fmt                              # Required before commit
cargo clippy --lib --tests --benches  # Fix all warnings
cargo test --lib                       # All library tests pass
cargo bench                            # Profile if performance impact
```

## PERFORMANCE

| Component | Pattern | Notes |
|-----------|---------|-------|
| Hashing | FNV-1a | Best speed/distribution balance |
| Locking | parking_lot::RwLock | Read-heavy workloads |
| Alignment | 64B x86_64, 128B ARM | Critical structures |
| Allocator | jemalloc | Global, configured |

## BUILD & TEST

```bash
cargo build --release                  # Optimized build
cargo test --lib                       # Library tests
cargo test -- --test-threads=1         # Single-threaded tests
cargo bench --bench hash_bench        # Hash performance
```

## MODULE-SPECIFIC GUIDES

- [buffer/AGENTS.md](src/buffer/AGENTS.md) - Buffer pool patterns
- [vfs/AGENTS.md](src/vfs/AGENTS.md) - VFS interface patterns
- [tablespace/AGENTS.md](src/tablespace/AGENTS.md) - Segment-page storage
- [table/AGENTS.md](src/table/AGENTS.md) - Table metadata & columns
- [catalog/AGENTS.md](src/catalog/AGENTS.md) - System catalog
- [vfs/AGENTS.md](src/vfs/AGENTS.md) - VFS interface patterns
- [tablespace/AGENTS.md](src/tablespace/AGENTS.md) - Segment-page storage

<skills_system priority="1">

## Available Skills

<!-- SKILLS_TABLE_START -->
<usage>
When users ask you to perform tasks, check if any of the available skills below can help complete the task more effectively. Skills provide specialized capabilities and domain knowledge.

How to use skills:
- Invoke: `npx openskills read <skill-name>` (run in your shell)
  - For multiple: `npx openskills read skill-one,skill-two`
- The skill content will load with detailed instructions on how to complete the task
- Base directory provided in output for resolving bundled resources (references/, scripts/, assets/)

Usage notes:
- Only use skills listed in <available_skills> below
- Do not invoke a skill that is already loaded in your context
- Each skill invocation is stateless
</usage>

<available_skills>

<skill>
<name>algorithmic-art</name>
<description>Creating algorithmic art using p5.js with seeded randomness and interactive parameter exploration. Use this when users request creating art using code, generative art, algorithmic art, flow fields, or particle systems. Create original algorithmic art rather than copying existing artists' work to avoid copyright violations.</description>
<location>global</location>
</skill>

<skill>
<name>brand-guidelines</name>
<description>Applies Anthropic's official brand colors and typography to any sort of artifact that may benefit from having Anthropic's look-and-feel. Use it when brand colors or style guidelines, visual formatting, or company design standards apply.</description>
<location>global</location>
</skill>

<skill>
<name>canvas-design</name>
<description>Create beautiful visual art in .png and .pdf documents using design philosophy. You should use this skill when the user asks to create a poster, piece of art, design, or other static piece. Create original visual designs, never copying existing artists' work to avoid copyright violations.</description>
<location>global</location>
</skill>

<skill>
<name>doc-coauthoring</name>
<description>Guide users through a structured workflow for co-authoring documentation. Use when user wants to write documentation, proposals, technical specs, decision docs, or similar structured content. This workflow helps users efficiently transfer context, refine content through iteration, and verify the doc works for readers. Trigger when user mentions writing docs, creating proposals, drafting specs, or similar documentation tasks.</description>
<location>global</location>
</skill>

<skill>
<name>docx</name>
<description>"Use this skill whenever the user wants to create, read, edit, or manipulate Word documents (.docx files). Triggers include: any mention of \"Word doc\", \"word document\", \".docx\", or requests to produce professional documents with formatting like tables of contents, headings, page numbers, or letterheads. Also use when extracting or reorganizing content from .docx files, inserting or replacing images in documents, performing find-and-replace in Word files, working with tracked changes or comments, or converting content into a polished Word document. If the user asks for a \"report\", \"memo\", \"letter\", \"template\", or similar deliverable as a Word or .docx file, use this skill. Do NOT use for PDFs, spreadsheets, Google Docs, or general coding tasks unrelated to document generation."</description>
<location>global</location>
</skill>

<skill>
<name>frontend-design</name>
<description>Create distinctive, production-grade frontend interfaces with high design quality. Use this skill when the user asks to build web components, pages, artifacts, posters, or applications (examples include websites, landing pages, dashboards, React components, HTML/CSS layouts, or when styling/beautifying any web UI). Generates creative, polished code and UI design that avoids generic AI aesthetics.</description>
<location>global</location>
</skill>

<skill>
<name>internal-comms</name>
<description>A set of resources to help me write all kinds of internal communications, using the formats that my company likes to use. Claude should use this skill whenever asked to write some sort of internal communications (status reports, leadership updates, 3P updates, company newsletters, FAQs, incident reports, project updates, etc.).</description>
<location>global</location>
</skill>

<skill>
<name>mcp-builder</name>
<description>Guide for creating high-quality MCP (Model Context Protocol) servers that enable LLMs to interact with external services through well-designed tools. Use when building MCP servers to integrate external APIs or services, whether in Python (FastMCP) or Node/TypeScript (MCP SDK).</description>
<location>global</location>
</skill>

<skill>
<name>pdf</name>
<description>Use this skill whenever the user wants to do anything with PDF files. This includes reading or extracting text/tables from PDFs, combining or merging multiple PDFs into one, splitting PDFs apart, rotating pages, adding watermarks, creating new PDFs, filling PDF forms, encrypting/decrypting PDFs, extracting images, and OCR on scanned PDFs to make them searchable. If the user mentions a .pdf file or asks to produce one, use this skill.</description>
<location>global</location>
</skill>

<skill>
<name>pptx</name>
<description>"Use this skill any time a .pptx file is involved in any way — as input, output, or both. This includes: creating slide decks, pitch decks, or presentations; reading, parsing, or extracting text from any .pptx file (even if the extracted content will be used elsewhere, like in an email or summary); editing, modifying, or updating existing presentations; combining or splitting slide files; working with templates, layouts, speaker notes, or comments. Trigger whenever the user mentions \"deck,\" \"slides,\" \"presentation,\" or references a .pptx filename, regardless of what they plan to do with the content afterward. If a .pptx file needs to be opened, created, or touched, use this skill."</description>
<location>global</location>
</skill>

<skill>
<name>skill-creator</name>
<description>Guide for creating effective skills. This skill should be used when users want to create a new skill (or update an existing skill) that extends Claude's capabilities with specialized knowledge, workflows, or tool integrations.</description>
<location>global</location>
</skill>

<skill>
<name>slack-gif-creator</name>
<description>Knowledge and utilities for creating animated GIFs optimized for Slack. Provides constraints, validation tools, and animation concepts. Use when users request animated GIFs for Slack like "make me a GIF of X doing Y for Slack."</description>
<location>global</location>
</skill>

<skill>
<name>template</name>
<description>Replace with description of the skill and when Claude should use it.</description>
<location>global</location>
</skill>

<skill>
<name>theme-factory</name>
<description>Toolkit for styling artifacts with a theme. These artifacts can be slides, docs, reportings, HTML landing pages, etc. There are 10 pre-set themes with colors/fonts that you can apply to any artifact that has been creating, or can generate a new theme on-the-fly.</description>
<location>global</location>
</skill>

<skill>
<name>web-artifacts-builder</name>
<description>Suite of tools for creating elaborate, multi-component claude.ai HTML artifacts using modern frontend web technologies (React, Tailwind CSS, shadcn/ui). Use for complex artifacts requiring state management, routing, or shadcn/ui components - not for simple single-file HTML/JSX artifacts.</description>
<location>global</location>
</skill>

<skill>
<name>webapp-testing</name>
<description>Toolkit for interacting with and testing local web applications using Playwright. Supports verifying frontend functionality, debugging UI behavior, capturing browser screenshots, and viewing browser logs.</description>
<location>global</location>
</skill>

<skill>
<name>xlsx</name>
<description>"Use this skill any time a spreadsheet file is the primary input or output. This means any task where the user wants to: open, read, edit, or fix an existing .xlsx, .xlsm, .csv, or .tsv file (e.g., adding columns, computing formulas, formatting, charting, cleaning messy data); create a new spreadsheet from scratch or from other data sources; or convert between tabular file formats. Trigger especially when the user references a spreadsheet file by name or path — even casually (like \"the xlsx in my downloads\") — and wants something done to it or produced from it. Also trigger for cleaning or restructuring messy tabular data files (malformed rows, misplaced headers, junk data) into proper spreadsheets. The deliverable must be a spreadsheet file. Do NOT trigger when the primary deliverable is a Word document, HTML report, standalone Python script, database pipeline, or Google Sheets API integration, even if tabular data is involved."</description>
<location>global</location>
</skill>

</available_skills>
<!-- SKILLS_TABLE_END -->

</skills_system>
