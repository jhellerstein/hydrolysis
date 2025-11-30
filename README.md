# Hydrolysis

A static analysis tool for Hydro IR that detects nondeterminism and CALM (Consistency As Logical Monotonicity) violations in dataflow programs.

## Overview

Hydrolysis analyzes Hydro IR JSON files exported by `hydro_lang::viz` and produces annotated JSON with analysis metadata. It performs two main analysis passes:

1. **ND Pass (Nondeterminism)**: Propagates nondeterminism taint via transitive closure over dataflow edges
2. **CALM Pass (Monotonicity)**: Checks that cross-location edges use lattice types and monotone operators

## Building

```bash
# Build in debug mode
cargo build

# Build optimized release version (recommended)
cargo build --release
```

The binary will be located at:
- Debug: `target/debug/hydrolysis`
- Release: `target/release/hydrolysis`

## Usage

```bash
# Run the analyzer
cargo run --release -- input.json output.json

# Or use the binary directly
./target/release/hydrolysis input.json output.json
```

### Input Format

The tool expects Hydro IR JSON with the following structure:

```json
{
  "nodes": [
    {
      "id": "1",
      "nodeType": "Source",
      "shortLabel": "my_source"
    }
  ],
  "edges": [
    {
      "id": "e1",
      "source": "1",
      "target": "2",
      "semanticTags": ["Local", "Stream"]
    }
  ]
}
```

See `.ref` for the complete JSON format specification.

### Output Format

The output JSON contains the same structure as the input, with added `analysis` fields on each node and edge, plus an `overall` summary:

```json
{
  "nodes": [
    {
      "id": "1",
      "nodeType": "Source",
      "shortLabel": "my_source",
      "analysis": {
        "nd_effect": "Deterministic",
        "monotone": true,
        "issues": []
      }
    }
  ],
  "edges": [...],
  "overall": {
    "deterministic": true,
    "calm_safe": true
  }
}
```

## Analysis Details

### Nondeterminism Analysis

- Identifies nodes with non-deterministic effects (e.g., `NonDeterministic` node type)
- Computes transitive closure to find all tainted downstream nodes
- Annotates each node with its ND effect: `Deterministic`, `LocallyNonDet`, or `ExternalNonDet`

### CALM Analysis

- Examines cross-location edges (edges with "Network" in `semanticTags`) and edges to Sink nodes
- Verifies all paths to these edges use monotone operators and lattice types
- Marks edges as `CalmSafe` or `CalmUnsafe`
- Computes overall `calm_safe` boolean for the entire program

### Issue Reporting

The tool generates three types of issues:

- **NonDet**: Node is nondeterministic
- **NonMonotone**: Non-monotone operator on a CALM-critical path
- **NonLattice**: Non-lattice type on a CALM-critical edge

## Testing

```bash
# Run all tests
cargo test

# Run with verbose output
cargo test -- --nocapture

# Run specific test
cargo test test_calm_safe_network_edge
```

The test suite includes both unit tests and property-based tests using `proptest`.

## Installing Globally

To install the tool globally:

```bash
cargo install --path .
```

Then you can run it from anywhere:

```bash
hydrolysis input.json output.json
```

## Project Structure

```
├── src/
│   ├── lib.rs           # Library root and re-exports
│   ├── model.rs         # JSON data structures
│   ├── semantics.rs     # Operator semantics table
│   ├── analysis.rs      # ND and CALM analysis passes
│   ├── annotate.rs      # Output annotation
│   └── bin/
│       └── main.rs      # CLI entrypoint
├── Cargo.toml           # Dependencies and project config
└── .ref                 # JSON format specification
```

## License

See LICENSE file for details.
