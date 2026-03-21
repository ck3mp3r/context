# Code Analysis Module

This module provides code analysis capabilities for c5t using:
- **Tree-sitter** - For parsing source code into ASTs
- **NanoGraph** - For storing and querying code graphs

## Requirements

### Build-time Dependencies

**Protocol Buffers Compiler (`protoc`)** - Required by NanoGraph's Lance dependency:

```bash
# macOS
brew install protobuf

# Linux (Ubuntu/Debian)
apt-get install protobuf-compiler

# Linux (Fedora)
dnf install protobuf-compiler

# Verify installation
protoc --version
```

**Without protoc installed, the backend feature will fail to compile.**

## Architecture

### Schema Design

- **2 node types:** File, Symbol (instead of 30+ specialized types)
- **5 edge types:** FileContains, SymbolContains, Calls, References, Inherits
- **Language-agnostic:** Same schema for all languages (Rust, TypeScript, Python, etc.)
- **Confidence tracking:** Relationships include confidence scores (0.0-1.0)

### Module Structure

```
src/analysis/
├── schema.pg           # NanoGraph schema definition
├── mod.rs              # Module exports
├── types.rs            # ExtractedSymbol, SymbolKind, etc.
├── types_test.rs       # Unit tests for types
├── parser.rs           # Tree-sitter wrapper
├── store.rs            # NanoGraph database wrapper
├── extractor.rs        # Generic extraction trait
└── languages/
    ├── mod.rs
    └── rust.rs         # Rust-specific symbol extraction
```

## Current Status

**MVP:** Rust language support only (dogfooding)

**Implemented:**
- [x] NanoGraph schema (schema.pg)
- [x] Type definitions (types.rs)
- [x] Tree-sitter parser wrapper (parser.rs)
- [x] Rust symbol extractor (languages/rust.rs)
- [ ] NanoGraph store implementation (store.rs - placeholder)
- [ ] MCP tools for analysis
- [ ] Tests (TDD - in progress)

## Usage (Planned)

```rust
use context::analysis::*;

// Parse Rust code
let mut parser = Parser::new_rust()?;
let tree = parser.parse(rust_code)?;

// Extract symbols
let symbols = RustExtractor::extract(rust_code);

// Store in graph (TODO: implement)
let mut graph = CodeGraph::new(db_path, repo_id).await?;
for symbol in symbols {
    graph.insert_symbol(&symbol).await?;
}
```

## CI/CD Note

GitHub Actions will need protoc installed:

```yaml
- name: Install protoc
  run: |
    brew install protobuf  # macOS
    # or
    sudo apt-get install -y protobuf-compiler  # Linux
```
