# Requirements Document

## Introduction

This document specifies requirements for a standalone Rust binary tool (`hydrolysis`) that performs static analysis on Hydro IR JSON. The tool analyzes dataflow programs for determinism and CALM (Consistency As Logical Monotonicity) properties, producing annotated JSON that can be visualized by Hydroscope without additional logic.

## Glossary

- **Hydrolysis**: The static analysis tool being specified (name derived from "Hydro analysis")
- **Hydro IR**: Intermediate Representation of a Hydro dataflow program, exported as JSON
- **CALM**: Consistency As Logical Monotonicity - a theorem stating that programs are eventually consistent without coordination iff they are monotonic
- **Nondeterminism (ND)**: Operations whose output may vary across executions given the same input
- **Monotonicity**: A property where adding more input data only adds to (never retracts from) the output
- **Lattice Type**: A data type with a merge operation that is associative, commutative, and idempotent (e.g., `SetUnion`, `CausalWrapper`)
- **Taint Propagation**: Tracking how nondeterminism spreads through the dataflow graph
- **Hydroscope**: The visualization tool that renders annotated Hydro IR

## Requirements

### Requirement 1: JSON Input Parsing

**User Story:** As a developer, I want to provide Hydro IR JSON as input, so that the tool can analyze my dataflow program.

#### Acceptance Criteria

1. WHEN Hydrolysis receives a file path argument THEN Hydrolysis SHALL read and parse the JSON file
2. WHEN the JSON contains a `nodes` array THEN Hydrolysis SHALL extract node `id`, `nodeType`, `label`, `fullLabel`, `shortLabel`, and `data` fields (including `data.locationId` and `data.locationType`)
3. WHEN the JSON contains an `edges` array THEN Hydrolysis SHALL extract edge `id`, `source`, `target`, `semanticTags`, and optional `label` fields
4. IF the JSON file is malformed or missing required fields THEN Hydrolysis SHALL report a descriptive error and exit with non-zero status
5. WHEN parsing completes successfully THEN Hydrolysis SHALL construct internal node and edge data structures for analysis

### Requirement 2: Operator Semantics Classification

**User Story:** As a developer, I want the tool to understand the semantics of Hydro operators, so that it can correctly classify their determinism and monotonicity properties.

#### Acceptance Criteria

1. WHEN Hydrolysis encounters a node THEN Hydrolysis SHALL look up its `nodeType` in a semantics table (valid types: Source, Transform, Join, Aggregation, Network, Sink, Tee, NonDeterministic)
2. WHEN an operator is classified THEN Hydrolysis SHALL assign one of three ND effects: Deterministic, LocallyNonDet, or ExternalNonDet
3. WHEN an operator is classified THEN Hydrolysis SHALL assign one of three monotonicity values: Always, Never, or Depends
4. WHEN the `nodeType` is "NonDeterministic" THEN Hydrolysis SHALL classify it as LocallyNonDet
5. WHEN the `nodeType` is unknown THEN Hydrolysis SHALL default to conservative assumptions (NonDet, Never)

### Requirement 3: Lattice Type Detection

**User Story:** As a developer, I want the tool to detect lattice types on edges, so that CALM analysis can determine consistency guarantees.

#### Acceptance Criteria

1. WHEN Hydrolysis examines an edge THEN Hydrolysis SHALL check its `semanticTags` array and optional `label` for lattice type indicators
2. WHEN the edge label contains "CausalWrapper", "VCWrapper", "DomPair", or "SetUnion" THEN Hydrolysis SHALL mark the edge as a lattice type
3. WHEN the edge `semanticTags` do not indicate a lattice type THEN Hydrolysis SHALL mark the edge as non-lattice

### Requirement 4: Nondeterminism Analysis

**User Story:** As a developer, I want to identify nondeterministic operations and their downstream effects, so that I can understand where my program may produce inconsistent results.

#### Acceptance Criteria

1. WHEN Hydrolysis performs ND analysis THEN Hydrolysis SHALL identify all nodes with non-deterministic effects
2. WHEN a nondeterministic node is identified THEN Hydrolysis SHALL compute transitive closure over outgoing edges to find all tainted nodes
3. WHEN taint propagation completes THEN Hydrolysis SHALL annotate each tainted node with its ND effect
4. WHEN a node is not tainted THEN Hydrolysis SHALL annotate it as Deterministic

### Requirement 5: CALM Monotonicity Analysis

**User Story:** As a developer, I want to verify CALM safety of my dataflow, so that I can ensure eventual consistency without coordination.

#### Acceptance Criteria

1. WHEN Hydrolysis performs CALM analysis THEN Hydrolysis SHALL examine all cross-location edges (edges with "Network" in `semanticTags`) and Sink nodes
2. WHEN analyzing an edge for CALM safety THEN Hydrolysis SHALL verify all paths to that edge use monotone operators and lattice types
3. WHEN all paths to an edge are monotone with lattice types THEN Hydrolysis SHALL mark the edge as CalmSafe
4. WHEN any path contains a non-monotone operator or non-lattice type THEN Hydrolysis SHALL mark the edge as CalmUnsafe
5. WHEN CALM analysis completes THEN Hydrolysis SHALL compute an overall calm_safe boolean for the entire program

### Requirement 6: Issue Reporting

**User Story:** As a developer, I want clear issue reports for analysis violations, so that I can fix problems in my dataflow program.

#### Acceptance Criteria

1. WHEN a nondeterminism issue is detected THEN Hydrolysis SHALL create an issue with kind "NonDet" and the affected node_id
2. WHEN a non-monotone operator is detected on a CALM-critical path THEN Hydrolysis SHALL create an issue with kind "NonMonotone"
3. WHEN a non-lattice type is detected on a CALM-critical edge THEN Hydrolysis SHALL create an issue with kind "NonLattice"
4. WHEN issues are generated THEN Hydrolysis SHALL include them in the analysis annotations for affected nodes and edges

### Requirement 7: Annotated JSON Output

**User Story:** As a developer, I want annotated JSON output, so that Hydroscope can visualize analysis results without additional logic.

#### Acceptance Criteria

1. WHEN analysis completes THEN Hydrolysis SHALL produce JSON with the same structure as input plus analysis annotations
2. WHEN annotating nodes THEN Hydrolysis SHALL add an "analysis" object with nd_effect, monotone, and issues fields
3. WHEN annotating edges THEN Hydrolysis SHALL add an "analysis" object with is_lattice, calm, and issues fields
4. WHEN writing output THEN Hydrolysis SHALL include an "overall" object with deterministic and calm_safe booleans
5. WHEN an output path is provided THEN Hydrolysis SHALL write pretty-printed JSON to that file
6. WHEN serializing the annotated JSON THEN Hydrolysis SHALL produce valid JSON that round-trips correctly

### Requirement 8: Command Line Interface

**User Story:** As a developer, I want a simple CLI, so that I can easily integrate the tool into my workflow.

#### Acceptance Criteria

1. WHEN Hydrolysis is invoked THEN Hydrolysis SHALL accept an input file path as the first argument
2. WHEN Hydrolysis is invoked THEN Hydrolysis SHALL accept an output file path as the second argument
3. IF required arguments are missing THEN Hydrolysis SHALL print usage information and exit with non-zero status
4. WHEN analysis succeeds THEN Hydrolysis SHALL exit with status 0
5. IF any error occurs THEN Hydrolysis SHALL print an error message to stderr and exit with non-zero status
