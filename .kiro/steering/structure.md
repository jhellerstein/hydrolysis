# Project Structure

```
├── src/
│   ├── lib.rs           # Library root, Hydro setup, re-exports
│   ├── model.rs         # JSON structs for Hydro IR (nodes, edges, analysis results)
│   ├── semantics.rs     # Operator semantics table (ND effect, monotonicity)
│   ├── analysis.rs      # Dataflow logic for ND and CALM analysis
│   └── annotate.rs      # Write annotated JSON output
├── src/bin/
│   └── main.rs          # CLI entrypoint (parse args, run analysis, write output)
├── examples/            # Example deployment scripts (if needed)
├── build.rs             # Stageleft code generation
├── Cargo.toml           # Dependencies and project config
└── .ref/hydro/          # Reference Hydro framework source
```

## Module Responsibilities

### `model.rs`
- `Node`, `Edge`, `HydroIr` structs matching JSON schema
- `NodeAnalysis`, `EdgeAnalysis` for output annotations
- Serde derive for JSON round-tripping

### `semantics.rs`
- `NdEffect` enum: `Deterministic`, `LocallyNonDet`, `ExternalNonDet`
- `Monotonicity` enum: `Always`, `Never`, `Depends`
- `OpSemantics` lookup by operator kind
- `is_lattice_type()` heuristic for type names

### `analysis.rs`
- **Hydro dataflows** that compute:
  - ND taint propagation (transitive closure from nondet sources)
  - CALM safety (backward reachability checking lattice + monotone paths)
  - Issue collection
- Input: streams of `Node` and `Edge`
- Output: streams of annotated nodes/edges and issues

### `annotate.rs`
- Merge analysis results back into original JSON structure
- Serialize annotated output

## Code Patterns

### Analysis as Dataflow
```rust
// Example: transitive closure for ND taint
let nd_sources = nodes.filter(q!(|n| n.nd_effect != NdEffect::Deterministic));
let tainted = transitive_closure(nd_sources, edges);
```

### Type Conventions
- `Stream<T, Process<'a, Analyzer>>` for analysis streams
- `KeyedStream<NodeId, T, ...>` when grouping by node
- Use `q!(...)` for all closure expressions
