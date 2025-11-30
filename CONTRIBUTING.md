# Contributing to Hydrolysis

Thank you for your interest in contributing to Hydrolysis!

## Development Setup

1. Install Rust (1.70 or later): https://rustup.rs/
2. Clone the repository:
   ```bash
   git clone https://github.com/jhellerstein/hydrolysis.git
   cd hydrolysis
   ```
3. Build the project:
   ```bash
   cargo build
   ```

## Running Tests

We use cargo-nextest for faster test execution:

```bash
# Install nextest
cargo install cargo-nextest

# Run tests
cargo nextest run

# Run with coverage
cargo install cargo-tarpaulin
cargo tarpaulin --all-features
```

## Code Quality

Before submitting a PR, please ensure:

1. **Tests pass**: `cargo nextest run`
2. **Code is formatted**: `cargo fmt`
3. **No clippy warnings**: `cargo clippy --all-targets -- -D warnings`
4. **Documentation builds**: `cargo doc --no-deps`

## Adding New Operators

To add semantics for a new Hydro operator:

1. Add the operator to `src/semantics.rs` in the `get_operator_semantics()` function
2. Specify its `NdEffect` (Deterministic, LocallyNonDet, or ExternalNonDet)
3. Specify its `Monotonicity` (Always, Never, or Depends)
4. Add tests if the semantics are non-obvious
5. Update documentation

## Property-Based Testing

We use proptest for property-based testing. When adding new features:

1. Consider what properties should hold
2. Add proptest cases in the relevant test module
3. Document the property being tested with a comment

## Commit Messages

Please use clear, descriptive commit messages:

- Start with a verb in present tense (e.g., "Add", "Fix", "Update")
- Keep the first line under 72 characters
- Add details in the body if needed

## Pull Request Process

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/my-feature`
3. Make your changes
4. Run tests and linters
5. Commit your changes
6. Push to your fork
7. Open a pull request

## Questions?

Feel free to open an issue for questions or discussions!
