# Frontend Architecture

The c5t frontend is a **single-page application (SPA)** built with Leptos and Thaw UI, compiled to WebAssembly and embedded directly into the `c5t` binary.

## Technology Stack

### Core Framework
- **Leptos** - Reactive UI framework for Rust/WASM
- **Thaw UI** - Component library (Table, Button, Layout, etc.)
- **Tachys** - Fine-grained reactive rendering system (Leptos's rendering layer)

### Build Tools
- **Trunk** - WASM web application bundler
- **wasm-bindgen** - Rust/WASM ↔ JavaScript interop
- **Tailwind CSS** - Utility-first CSS framework
- **rust-embed** - Compile-time asset embedding

### Browser APIs
- **Web Storage API** - localStorage for client-side persistence
- **Fetch API** - HTTP requests to backend REST API
- **WebSocket API** - Real-time bidirectional communication
- **History API** - Client-side routing (SPA navigation)

## Project Structure

```
src/frontend/
├── main.rs              # WASM entry point (wasm_bindgen)
├── app.rs               # Root component + routing
├── api/                 # HTTP client for backend API
│   └── mod.rs
├── components/          # Reusable UI components
│   ├── mod.rs
│   ├── task_components.rs   # Task/subtask display
│   ├── note_components.rs   # Note list/detail views
│   └── ui_components.rs     # Copyable ID, etc.
├── models/              # Frontend data models
│   └── mod.rs
├── pages/               # Page-level components
│   ├── mod.rs
│   ├── projects.rs      # Project list
│   ├── project_detail.rs
│   ├── notes.rs         # Note list/search
│   └── repos.rs
└── assets/
    ├── index.html       # HTML template
    ├── input.css        # Tailwind CSS input
    ├── style.css        # Generated CSS (gitignored)
    └── tailwind.config.js
```

## Development Workflow

### Development Mode

Run **two processes** for hot reloading:

1. **Trunk dev server** (port 8080):
   ```sh
   trunk serve
   ```
   - Watches `src/frontend/**` for changes
   - Auto-rebuilds WASM on file save
   - Live reloads browser
   - Proxies `/dev/api/*` → `http://localhost:3737/api/*`

2. **Backend API server** (port 3737):
   ```sh
   cargo run --bin c5t -- api --port 3737
   ```
   - Serves REST API at `/api/v1/*`
   - Serves MCP at `/mcp`

**Access the frontend**: http://localhost:8080/ (served by Trunk)

**IMPORTANT**: In dev mode, the frontend uses `/dev/api/v1` for API calls (proxied by Trunk). Do NOT access `http://localhost:3737/` directly - it serves stale `dist/` assets.

### Production Build

The production build uses a **two-stage Nix build** process:

#### Stage 1: Frontend Assets (`flake.nix`)
```nix
frontendAssets = pkgs.rustPlatform.buildRustPackage {
  # ... rustc toolchain with wasm32-unknown-unknown target
  buildPhase = ''
    export HOME=$TMPDIR  # wasm-bindgen cache workaround
    trunk build --release
  '';
  installPhase = ''
    cp -r dist $out
  '';
};
```

This derivation:
- Uses WASM-enabled Rust toolchain
- Runs `trunk build --release` to compile frontend
- Outputs compiled assets to Nix store

#### Stage 2: Backend with Embedded Assets
```nix
srcWithFrontend = pkgs.runCommand "src-with-frontend" {} ''
  cp -r ${src} $out
  chmod -R u+w $out
  cp -r ${frontendAssets} $out/dist
'';
```

Then the main build:
- Uses `srcWithFrontend` (source + pre-built frontend)
- `rust-embed` finds `dist/` at compile time
- Generates `Embed` trait implementation
- Embeds assets into final binary with compression

**Manual production build** (outside Nix):
```sh
trunk build --release   # Compile frontend to dist/
cargo build --release   # rust-embed finds dist/, embeds assets
./target/release/c5t api --port 3737
```

## Embedded Assets Strategy

### rust-embed Configuration

`src/api/static_assets.rs`:
```rust
#[derive(RustEmbed)]
#[folder = "dist/"]
#[include = "*.html"]
#[include = "*.js"]
#[include = "*.wasm"]
#[include = "*.css"]
#[include = "snippets/**/*"]  # wasm-bindgen snippets
struct FrontendAssets;
```

**Debug mode**: `rust-embed` reads from `dist/` at runtime (filesystem)
**Release mode**: Assets embedded at compile time with compression

### SPA Routing Logic

The `serve_frontend()` function handles routing:

1. **Skip backend routes**: `/api/*`, `/mcp`, `/docs` → return 404 (handled by other routers)
2. **Exact file match**: `/app.wasm`, `/style.css` → serve file
3. **SPA fallback**: `/notes`, `/projects/*` → serve `index.html` (client-side routing)
4. **Error**: If `index.html` not found → 500 (should never happen)

### Cache Headers Strategy

- **Hashed assets** (e.g., `app-a1b2c3d4.wasm`):
  ```
  Cache-Control: public, max-age=31536000  # 1 year
  ```
  Safe to cache forever - hash changes on content change

- **index.html**:
  ```
  Cache-Control: no-cache
  ```
  Always revalidate - ensures users get latest SPA version

## API Integration

### Development Mode
Frontend uses `/dev/api/v1` prefix (configured in `Trunk.toml` proxy):
```toml
[[proxy]]
backend = "http://localhost:3737"
rewrite = "/dev"
```

Request flow: `fetch('/dev/api/v1/projects')` → Trunk proxy → `http://localhost:3737/api/v1/projects`

### Production Mode
Frontend uses `/api/v1` prefix directly (same origin, no CORS):
```rust
let projects = fetch("/api/v1/projects").await?;
```

## Client-Side Routing

Leptos Router configuration (`src/frontend/app.rs`):

```rust
<Router>
    <Route path="/" view=ProjectsPage />
    <Route path="/projects/:id" view=ProjectDetailPage />
    <Route path="/notes" view=NotesPage />
    <Route path="/repos" view=ReposPage />
</Router>
```

The backend's SPA fallback ensures these routes work:
- User navigates to `/notes`
- Backend serves `index.html`
- WASM loads, Leptos router matches `/notes`
- Renders `NotesPage` component

## Build Configuration

### Trunk (`Trunk.toml`)

```toml
[build]
target = "src/frontend/assets/index.html"
dist = "dist"
public_url = "/"
filehash = true           # Hash assets for cache busting
minify = "on_release"     # Minify JS/CSS in release
wasm_opt = "off"          # Disabled (cargo already optimizes)
target_dir = "target/trunk"  # Separate from cargo target/

[[hooks]]
stage = "pre_build"
command = "tailwindcss"   # Generate CSS before WASM build
```

### Cargo Features

Frontend-only dependencies use `default-features = false`:
```toml
[dependencies]
leptos = { version = "0.7", default-features = false, features = ["csr"] }
thaw = { version = "0.4", default-features = false }
```

This avoids pulling in server-side rendering deps for WASM builds.

## Known Issues & Workarounds

### RefCell Borrow Panic (Task #6795f53e)

**Symptom**: `BorrowMutError` in `CopyableId` component when accessing via `0.0.0.0`

**Cause**: Browser security context differences cause async clipboard API timing issues

**Workaround**: Access via `localhost` or `127.0.0.1` instead of `0.0.0.0`

**Location**: `src/frontend/components/ui_components.rs:13-50`

### Trunk Cache Stale Builds

**Symptom**: `trunk serve` uses old WASM after running `trunk build --release`

**Cause**: Trunk caches compiled WASM

**Fix**: 
```sh
cargo clean
rm -rf target/ dist/
trunk serve  # Fresh rebuild
```

## Testing

### Frontend Tests
Currently minimal - future work to add:
- Component unit tests (Leptos testing utilities)
- Integration tests (playwright/selenium)

### Backend Static Asset Tests
See `src/api/static_assets.rs`:
- Root serves `index.html`
- API routes return 404 (handled elsewhere)
- SPA fallback works for unknown routes

## Performance Considerations

### Binary Size
- **WASM size**: ~13MB uncompressed → ~2-3MB with rust-embed compression
- **Final binary**: ~4-5MB total (backend + embedded WASM)

### Optimization Flags
`Cargo.toml`:
```toml
[profile.release]
opt-level = "z"      # Optimize for size
lto = true           # Link-time optimization
codegen-units = 1    # Single codegen unit (better optimization)
strip = true         # Strip debug symbols
```

### Lazy Loading
Future work: Code-split WASM by route for faster initial load

## References

- [Leptos Book](https://leptos-rs.github.io/leptos/)
- [Thaw UI Components](https://thaw-ui.vercel.app/)
- [Trunk Guide](https://trunkrs.dev/)
- [rust-embed Docs](https://docs.rs/rust-embed/)
