# Code Analysis Module

This module provides code analysis capabilities for c5t using:
- **Tree-sitter** - For parsing source code into ASTs
- **NanoGraph** - For storing and querying code graphs via CLI

## Requirements

### Runtime Dependencies

**NanoGraph CLI** - Required for graph operations:

```bash
# macOS
brew install nanograph/tap/nanograph

# Verify installation
nanograph --version
```

**Why CLI instead of library?** NanoGraph as a library adds 400MB+ to the binary. Shelling out to the CLI keeps the binary small while providing full functionality.

## Architecture

### Type System Design

**Three distinct type levels:**

1. **Language-specific kinds** (`lang::rust::Kind`, `lang::typescript::Kind`, etc.)
   - Language-specific symbol types (e.g., Rust has `Mod`, `Const`, `Static`)
   - Implements `Into<types::Kind>` for conversion to generic kinds

2. **Generic kinds** (`types::Kind`)
   - Language-agnostic categories (Function, Class, Struct, Trait, etc.)
   - Used for storage and queries
   - Serializes to/from string for database

3. **Symbol struct** (`types::Symbol`)
   - Unified struct for both insertion and queries
   - Contains: name, kind (generic Kind enum), language, file_path, lines, signature, content
   - Content field empty during insertion, filled during queries

**Type flow:**
```
Parse:  lang::rust::Kind -> .into() -> types::Kind
Store:  types::Kind -> .as_str() -> "function" (DB)
Query:  "function" -> .parse() -> types::Kind
```

### Language Trait Pattern

```rust
pub trait Language {
    type Kind: AsRef<str> + Clone + Into<types::Kind>;
    
    fn grammar() -> tree_sitter::Language;
    fn parse_symbol(node: Node, code: &str) -> Option<(Self::Kind, String)>;
    fn extract_callee(node: Node, code: &str) -> Option<String>;
    fn name() -> &'static str;
    fn extensions() -> &'static [&'static str];
}

// Generic parser works with any Language
pub struct Parser<L: Language> { ... }
```

### Schema Design

- **2 node types:** File, Symbol
- **4 edge types:** FileContains, Calls, References, Inherits
- **Language-specific kinds stored as strings** in Symbol.kind field
- **Confidence tracking:** Relationships include confidence scores (0.0-1.0)

### Module Structure

```
src/analysis/
├── schema.pg              # NanoGraph schema definition
├── mod.rs                 # Module exports
├── types.rs               # Symbol struct, generic Kind enum
├── parser.rs              # Language trait, generic Parser<L>
├── store.rs               # NanoGraph CLI wrapper (CodeGraph)
├── service.rs             # Analysis service with macro-based language registry
├── integration_test.rs    # Full analysis pipeline tests
├── store_test.rs          # Store operations tests
└── lang/
    └── rust/
        ├── mod.rs         # Exports
        ├── types.rs       # rust::Kind enum
        └── parser.rs      # impl Language for Rust
```

### Macro-Based Language Registry

**Single place to register new languages:**

```rust
// In service.rs
languages! {
    Rust,
    // TypeScript,  // Add here when ready
    // Python,      // Add here when ready
}
```

Generates compile-time dispatch for:
- File extension matching (`can_handle!` macro)
- Analysis routing (`analyze!` macro)

**No dynamic dispatch, zero runtime overhead!**

## Current Status

**Implemented:**
- [x] NanoGraph schema (2 nodes, 4 edges)
- [x] Type system (Symbol, Kind, language-specific kinds)
- [x] Language trait with generic parser
- [x] Rust language support (functions, structs, enums, traits, consts, statics, types, mods)
- [x] Single-pass AST walk (inserts symbols + relationships)
- [x] Call graph extraction (Calls edges)
- [x] NanoGraph CLI wrapper (batch inserts, commit, query)
- [x] Macro-based language registry
- [x] 611 passing tests
- [ ] Methods in impl blocks
- [ ] References edges (type annotations, imports)
- [ ] Inherits edges (trait bounds, implements)
- [ ] TypeScript support
- [ ] MCP tools for analysis

## Usage

### Analyze a file

```rust
use context::analysis::{Parser, Rust, CodeGraph};

// Create database
let mut graph = CodeGraph::new(db_path, repo_id).await?;

// Parse and analyze Rust code (single pass!)
let mut parser = Parser::<Rust>::new();
let stats = parser.parse_and_analyze(code, file_path, &mut graph).await?;

// Commit to database
graph.commit().await?;

println!("Inserted {} symbols, {} relationships", 
    stats.symbols_inserted, 
    stats.relationships_inserted);
```

### Query symbols

```rust
// Query symbols in a file
let symbols = graph.query_symbols_in_file("src/main.rs").await?;

for symbol in symbols {
    println!("{} ({}) at {}:{}-{}", 
        symbol.name, 
        symbol.kind.as_str(),  // Uses types::Kind enum
        symbol.file_path,
        symbol.start_line,
        symbol.end_line
    );
}
```

### Adding a new language

1. Create `src/analysis/lang/newlang/` directory
2. Define `lang::newlang::Kind` enum with language-specific kinds
3. Implement `From<newlang::Kind> for types::Kind`
4. Implement `Language` trait for your language type
5. Add to `languages!` macro in `service.rs`

Done! Compile-time type checking ensures correctness.

## Design Decisions

1. **Language-specific types over universal nodes** - Better type safety, clearer intent
2. **Macro registry over runtime dispatch** - Zero overhead, compile-time checks
3. **Single walk, HashMap tracking** - One AST pass inserts everything
4. **Generic parser with trait bounds** - Code reuse without sacrificing type safety
5. **CLI wrapper over library** - 400MB+ savings in binary size
6. **Type-safe conversions** - Use `From`/`Into` traits, not strings everywhere
