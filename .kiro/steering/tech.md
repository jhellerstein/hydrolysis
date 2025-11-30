# Tech Stack

## Language & Edition
- Rust 2024 edition

## Core Dependencies
- `hydro_lang`: Core Hydro language — used to implement analysis dataflows
- `hydro_std`: Standard library utilities for Hydro
- `stageleft` / `stageleft_tool`: Staged metaprogramming for code generation
- `serde` / `serde_json`: JSON parsing and serialization
- `anyhow`: Error handling

## Build System
- Cargo (Rust's package manager)
- Custom `build.rs` using `stageleft_tool::gen_final!()`

## Common Commands

```bash
# Build
cargo build --all-targets

# Run tests
cargo test

# Run specific test
cargo test -- path::to::test_name

# Run the analyzer (planned)
cargo run -- input.json output.json

# Format and lint
cargo fmt
cargo clippy
```

## Testing
- Uses Hydro's simulation framework (`flow.sim().exhaustive()`)
- Tests use `sim_input()` and `sim_output()` for deterministic I/O
- Assertions: `assert_yields_only()`, `assert_yields_unordered()`, `assert_yields_only_unordered()`
