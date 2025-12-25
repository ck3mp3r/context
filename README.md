# context

A Rust project with CLI and API binaries sharing a common library.

## Structure

```
context/
├── src/
│   ├── lib.rs          # Shared library
│   └── bin/
│       ├── cli.rs      # CLI binary
│       └── api.rs      # API binary
├── docs/               # Documentation
└── nix/                # Nix configuration
```

## Development

### Prerequisites

- [Nix](https://nixos.org/) with flakes enabled
- [direnv](https://direnv.net/) (optional, for automatic shell activation)

### Setup

```sh
# With direnv
direnv allow

# Without direnv
nix develop
```

### Commands

```sh
cargo build           # Build all binaries
cargo test            # Run tests
cargo run --bin c5t       # Run CLI
cargo run --bin c5t-api   # Run API
```

## License

MIT
