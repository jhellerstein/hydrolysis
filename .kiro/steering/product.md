# Product Overview

`hydro-analyze` is a static analysis tool for Hydro IR that detects nondeterminism and CALM monotonicity violations.

## Purpose
- Ingest Hydro IR JSON exported by `hydro_lang::viz`
- Run determinism and CALM monotonicity analysis
- Output annotated JSON with analysis metadata for visualization in Hydroscope

## Key Analysis Passes
1. **ND Pass (Nondeterminism)**: Propagate nondeterminism taint via transitive closure over dataflow edges
2. **CALM Pass (Monotonicity)**: Check that cross-location edges use lattice types and monotone operators
3. **Issue Extraction**: Generate warnings for NonDet, NonMonotone, and NonLattice violations

## Design Philosophy
The analysis logic itself is implemented as Hydro dataflows, using the framework's join/map/filter operators to express graph algorithms in a Datalog-like style. This dogfoods Hydro and leverages its natural fit for recursive graph computations (transitive closure, reachability).

## Input/Output
- **Input**: Raw Hydro IR JSON with nodes (operators) and edges (data flow)
- **Output**: Same JSON annotated with `analysis` fields per node/edge, plus overall summary
