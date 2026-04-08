# Nushell Analysis Support

## What It Does

Analyzes `.nu` files to extract commands, modules, imports, and call relationships into the code graph.

## Key Features

- **Symbol extraction**: Commands, modules, visibility, **test functions**
- **Import resolution**: Resolves `use` statements to target symbols
- **Glob imports**: `use foo *` correctly expands to all symbols in module
- **Module paths**: Derived from file structure (`tools/k8s/utils.nu` → `k8s::utils`)
- **Call tracking**: Function/command call relationships
- **Test detection**: Automatically identifies test functions following Nushell conventions

## Test Detection

### How It Works

Test functions are automatically identified by:

1. **Name pattern**: Function name starts with `test ` (space), `test-`, or `test_`
2. **Empty parameters**: Test functions must have empty parameter list `[]`
3. **Loose mode**: Both exported (`export def`) and private (`def`) tests are detected

### Examples

```nushell
# ✓ Detected as test
export def "test fibonacci" [] {
    assert equal (fib 5) 5
}

# ✓ Detected as test (kebab-case)
export def test-addition [] {
    assert equal (1 + 1) 2
}

# ✓ Detected as test (snake_case)
def test_subtraction [] {
    assert equal (5 - 3) 2
}

# ✗ NOT a test (has parameters)
export def "test runner" [name: string] {
    print $"Running: ($name)"
}

# ✗ NOT a test (doesn't start with test pattern)
def calculate [] { 42 }
```

### Symbol Kind

Test functions are assigned `kind: "test"` in the code graph, distinct from:
- `kind: "function"` - Regular functions
- `kind: "command"` - Space-separated command names or `main`
- `kind: "module"` - Module definitions

## Import Resolution

### How It Works

1. Extract `use` statements from AST
2. Normalize path: `lib/math.nu` → `lib::math` (replace `/` with `::`, strip `.nu`)
3. Look up symbols in SymbolRegistry:
   - Named: `use foo [bar, baz]` → find `foo::bar`, `foo::baz`
   - Glob: `use foo *` → find all symbols in `foo` module
4. Create FileImports edges in graph

### Example

```nushell
# tools/k8s/mod.nu
use utils.nu *
use formatters.nu [get-all-schemas]
```

**Creates FileImports edges:**
```
File(tools/k8s/mod.nu) → Symbol(format-tool-response)  # from utils.nu *
File(tools/k8s/mod.nu) → Symbol(run-kubectl)           # from utils.nu *
File(tools/k8s/mod.nu) → Symbol(get-all-schemas)       # explicit import
... (27 total)
```

## Module Path Mapping

| File | Module Path | Symbol Qualified Name |
|------|-------------|---------------------|
| `tools/k8s/mod.nu` | `k8s` | `k8s::main` |
| `tools/k8s/utils.nu` | `k8s::utils` | `k8s::utils::format-tool-response` |
| `tools/gh/formatters.nu` | `gh::formatters` | `gh::formatters::format-relative-time` |

## Implementation

### Key Files

- `src/a6s/lang/nushell/extractor.rs` - Symbol extraction, import resolution
- `src/a6s/lang/nushell/extractor_test.rs` - Unit tests
- `src/a6s/pipeline.rs` - Calls `resolve_imports()` for each extractor
- `src/a6s/lang/nushell/queries/symbols.scm` - Tree-sitter symbol queries
- `src/a6s/lang/nushell/queries/imports.scm` - Tree-sitter import queries

### Key Functions

- `extract_imports()` - Parse `use` statements from AST
- `resolve_imports()` - Match imports to SymbolRegistry, return ResolvedImport list
- `normalise_import_path()` - Convert `lib/math.nu` → `lib::math`

## Gotchas

### Glob Import AST Node

The AST node for `use foo *` is **`scope_pattern`** (with text `*`), NOT `wild_card`.

```rust
// WRONG
if child.kind() == "wild_card" { ... }

// CORRECT
if child.kind() == "scope_pattern" && child_text == "*" { ... }
```

### Path Separator

Nushell uses `/` in imports but module paths use `::`:

```nushell
use lib/math.nu *  # Nushell syntax
```

Internally we use: `lib::math` (module path with `::`)

## Testing

### Run Tests

```bash
cargo test --lib nushell          # All Nushell tests
cargo test --lib test_nushell_multi_file_integration  # Integration test
```

### Test Coverage

- Symbol extraction (commands, modules, visibility, **test functions**)
- **Test function detection** (space, kebab-case, snake_case patterns)
- **Parameter validation** (tests must have empty `[]`)
- Import extraction (named, glob)
- Import resolution with SymbolRegistry
- Glob import AST detection
- Path normalization
- End-to-end multi-file integration

### Real-World Validation

Tested on [nu-mcp](https://github.com/ck3mp3r/nu-mcp) repo:
- 50+ `.nu` files analyzed
- 305 commands extracted (227 public, 78 private)
- 119 FileImports edges from Nushell files
- Hub symbols correctly identified (most-called commands)

## Limitations

- No stdlib import tracking (`use std *` not resolved to stdlib symbols)
- No alias tracking (`use foo as bar` - alias name not tracked)
- No multi-file re-export chains
- No dynamic/conditional imports

## Future Work

- Track import aliases
- Add stdlib predefined symbols
- Detect circular imports
- Suggest imports for unresolved calls
