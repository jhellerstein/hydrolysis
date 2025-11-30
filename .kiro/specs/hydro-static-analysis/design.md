# Design Document: Hydrolysis

## Overview

Hydrolysis is a standalone Rust binary that performs static analysis on Hydro IR JSON files. It analyzes dataflow programs for determinism (nondeterminism taint propagation) and CALM (Consistency As Logical Monotonicity) properties, producing annotated JSON that Hydroscope can visualize without additional logic.

The tool reads JSON exported by `hydro_lang::viz`, runs analysis passes implemented as graph algorithms, and outputs the same JSON structure with added `analysis` metadata on nodes and edges.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         Hydrolysis                               │
├─────────────────────────────────────────────────────────────────┤
│  main.rs                                                         │
│  ├── CLI argument parsing                                        │
│  └── Orchestration                                               │
├─────────────────────────────────────────────────────────────────┤
│  model.rs                                                        │
│  ├── HydroIr (input JSON structure)                             │
│  ├── Node, Edge, NodeData                                        │
│  └── AnnotatedHydroIr (output JSON structure)                   │
├─────────────────────────────────────────────────────────────────┤
│  semantics.rs                                                    │
│  ├── NdEffect enum (Deterministic, LocallyNonDet, ExternalNonDet)│
│  ├── Monotonicity enum (Always, Never, Depends)                  │
│  ├── OpSemantics struct                                          │
│  └── Operator semantics lookup table                             │
├─────────────────────────────────────────────────────────────────┤
│  analysis.rs                                                     │
│  ├── ND Pass (nondeterminism taint propagation)                 │
│  ├── CALM Pass (monotonicity + lattice analysis)                │
│  └── Issue extraction                                            │
├─────────────────────────────────────────────────────────────────┤
│  annotate.rs                                                     │
│  ├── Merge analysis results into JSON                           │
│  └── Serialize annotated output                                  │
└─────────────────────────────────────────────────────────────────┘
```

## Components and Interfaces

### 1. Model Module (`model.rs`)

Defines the data structures for JSON input/output.

```rust
// Input structures (matching hydro_lang::viz output)
pub struct HydroIr {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    // Other fields preserved but not analyzed
    pub hierarchy_choices: Option<serde_json::Value>,
    pub node_assignments: Option<serde_json::Value>,
    pub selected_hierarchy: Option<String>,
    pub edge_style_config: Option<serde_json::Value>,
    pub node_type_config: Option<serde_json::Value>,
    pub legend: Option<serde_json::Value>,
}

pub struct Node {
    pub id: String,
    pub node_type: String,      // "Source", "Transform", "Join", etc.
    pub label: String,
    pub full_label: String,
    pub short_label: String,
    pub data: NodeData,
}

pub struct NodeData {
    pub location_id: Option<usize>,
    pub location_type: Option<String>,
    pub backtrace: serde_json::Value,
}

pub struct Edge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub semantic_tags: Vec<String>,  // "Bounded", "Unbounded", "Network", "Local", "Stream", "Keyed", "TotalOrder", "NoOrder", etc.
    // Note: label field is rarely present in current viz output
}

// Output structures (input + analysis annotations)
pub struct NodeAnalysis {
    pub nd_effect: String,      // "Deterministic", "LocallyNonDet", "ExternalNonDet"
    pub monotone: bool,
    pub issues: Vec<Issue>,
}

pub struct EdgeAnalysis {
    pub is_lattice: bool,
    pub calm: String,           // "CalmSafe", "CalmUnsafe"
    pub issues: Vec<Issue>,
}

pub struct Issue {
    pub kind: String,           // "NonDet", "NonMonotone", "NonLattice"
    pub message: String,
}

pub struct OverallAnalysis {
    pub deterministic: bool,
    pub calm_safe: bool,
}
```

### 2. Semantics Module (`semantics.rs`)

Provides operator classification based on node type.

```rust
pub enum NdEffect {
    Deterministic,
    LocallyNonDet,
    ExternalNonDet,
}

pub enum Monotonicity {
    Always,
    Never,
    Depends,
}

pub struct OpSemantics {
    pub nd: NdEffect,
    pub monotone: Monotonicity,
}

// Lookup table for node types (based on hydro_lang::viz::render.rs mappings)
// 
// Node Type Mappings from Hydro IR:
// - Source: source_iter, source_stream, cycle_source, external_network
// - Transform: map, filter, flat_map, filter_map, inspect, cast, defer_tick,
//              enumerate, unique, resolve_futures, resolve_futures_ordered,
//              persist, chain, chain_first, counter
// - Join: join, cross_product, cross_singleton, difference, anti_join,
//         reduce_keyed_watermark (join portion)
// - Aggregation: sort, reduce, reduce_keyed, fold, fold_keyed, scan,
//                reduce_keyed_watermark (aggregation portion)
// - Network: network (send/recv)
// - Sink: for_each, send_external, dest_sink, cycle_sink
// - Tee: tee (branch dataflow)
// - NonDeterministic: observe_non_det, batch
//
pub fn get_semantics(node_type: &str) -> OpSemantics {
    match node_type {
        "Source" => OpSemantics { nd: Deterministic, monotone: Always },
        "Transform" => OpSemantics { nd: Deterministic, monotone: Always },
        "Join" => OpSemantics { nd: Deterministic, monotone: Always },
        "Aggregation" => OpSemantics { nd: Deterministic, monotone: Depends },
        "Network" => OpSemantics { nd: Deterministic, monotone: Always },
        "Sink" => OpSemantics { nd: Deterministic, monotone: Always },
        "Tee" => OpSemantics { nd: Deterministic, monotone: Always },
        "NonDeterministic" => OpSemantics { nd: LocallyNonDet, monotone: Never },
        _ => OpSemantics { nd: LocallyNonDet, monotone: Never }, // Conservative default
    }
}

// For finer-grained analysis, we can also inspect the node label (shortLabel/label)
// to determine specific operator semantics:
pub fn get_semantics_by_label(label: &str) -> Option<OpSemantics> {
    match label.to_lowercase().as_str() {
        // Monotone transforms
        "map" | "filter" | "flat_map" | "filter_map" | "inspect" => 
            Some(OpSemantics { nd: Deterministic, monotone: Always }),
        
        // Monotone joins (set operations)
        "join" | "cross_product" | "cross_singleton" =>
            Some(OpSemantics { nd: Deterministic, monotone: Always }),
        
        // Non-monotone operations (require negation/retraction)
        "difference" | "anti_join" =>
            Some(OpSemantics { nd: Deterministic, monotone: Never }),
        
        // Aggregations - monotonicity depends on the aggregation function
        "reduce" | "reduce_keyed" | "fold" | "fold_keyed" | "scan" | "sort" =>
            Some(OpSemantics { nd: Deterministic, monotone: Depends }),
        
        // Explicitly non-deterministic
        "observe_non_det" | "batch" | "nondet" =>
            Some(OpSemantics { nd: LocallyNonDet, monotone: Never }),
        
        // State operations - monotone if lattice-based
        "persist" =>
            Some(OpSemantics { nd: Deterministic, monotone: Depends }),
        
        _ => None, // Fall back to node_type-based lookup
    }
}

// Lattice type detection from edge labels
pub fn is_lattice_type(label: Option<&str>) -> bool {
    label.map_or(false, |l| {
        l.contains("CausalWrapper")
            || l.contains("VCWrapper")
            || l.contains("DomPair")
            || l.contains("SetUnion")
            || l.contains("MapUnion")
            || l.contains("Max")
            || l.contains("Min")
    })
}
```

### 3. Analysis Module (`analysis.rs`)

Implements the core analysis passes.

```rust
pub struct AnalysisResult {
    pub node_analyses: HashMap<String, NodeAnalysis>,
    pub edge_analyses: HashMap<String, EdgeAnalysis>,
    pub overall: OverallAnalysis,
}

pub fn run_analysis(ir: &HydroIr) -> AnalysisResult {
    // Build adjacency lists for graph traversal
    let graph = build_graph(ir);
    
    // Pass 1: ND taint propagation
    let nd_results = run_nd_pass(&graph, ir);
    
    // Pass 2: CALM analysis
    let calm_results = run_calm_pass(&graph, ir);
    
    // Combine results
    combine_results(nd_results, calm_results)
}
```

#### ND Pass Algorithm

```
1. Identify seed nodes: nodes where semantics.nd != Deterministic
2. Initialize taint set with seed nodes
3. Compute transitive closure over outgoing edges:
   - For each tainted node, add all successors to taint set
   - Repeat until no new nodes are added
4. Annotate all tainted nodes with their ND effect
5. Annotate untainted nodes as Deterministic
```

#### CALM Pass Algorithm

```
1. Identify CALM-critical edges:
   - Edges with "Network" in semantic_tags
   - Edges targeting Sink nodes
2. For each critical edge:
   a. Compute backward reachability to find all paths
   b. For each path, check:
      - All nodes are monotone (semantics.monotone != Never)
      - All edges are lattice types
   c. Mark edge as CalmSafe if all paths pass, CalmUnsafe otherwise
3. Compute overall calm_safe = all critical edges are CalmSafe
```

### 4. Annotate Module (`annotate.rs`)

Merges analysis results into the original JSON structure.

```rust
pub fn annotate(ir: HydroIr, results: AnalysisResult) -> AnnotatedHydroIr {
    // Clone original structure
    let mut annotated = ir.clone();
    
    // Add analysis to each node
    for node in &mut annotated.nodes {
        if let Some(analysis) = results.node_analyses.get(&node.id) {
            node.analysis = Some(analysis.clone());
        }
    }
    
    // Add analysis to each edge
    for edge in &mut annotated.edges {
        if let Some(analysis) = results.edge_analyses.get(&edge.id) {
            edge.analysis = Some(analysis.clone());
        }
    }
    
    // Add overall summary
    annotated.overall = Some(results.overall);
    
    annotated
}
```

## Data Models

### Input JSON Schema

Based on actual `hydro_lang::viz` output snapshots:

```json
{
  "nodes": [
    {
      "id": "0",
      "nodeType": "Source",
      "label": "source",
      "fullLabel": "source [hydro operator]",
      "shortLabel": "source",
      "data": {
        "locationId": 0,
        "locationType": "Process",
        "backtrace": []
      }
    }
  ],
  "edges": [
    {
      "id": "e0",
      "source": "0",
      "target": "1",
      "semanticTags": ["Local", "Stream", "TotalOrder", "Unbounded"]
    }
  ],
  "hierarchyChoices": [
    { "id": "location", "name": "Location", "children": [] }
  ],
  "nodeAssignments": {
    "location": { "0": "loc_0", "1": "loc_0" }
  },
  "selectedHierarchy": "location",
  "edgeStyleConfig": { ... },
  "nodeTypeConfig": {
    "defaultType": "Transform",
    "types": [
      { "id": "Aggregation", "label": "Aggregation", "colorIndex": 0 },
      { "id": "Join", "label": "Join", "colorIndex": 1 },
      { "id": "Network", "label": "Network", "colorIndex": 2 },
      { "id": "NonDeterministic", "label": "NonDeterministic", "colorIndex": 3 },
      { "id": "Sink", "label": "Sink", "colorIndex": 4 },
      { "id": "Source", "label": "Source", "colorIndex": 5 },
      { "id": "Tee", "label": "Tee", "colorIndex": 6 },
      { "id": "Transform", "label": "Transform", "colorIndex": 7 }
    ]
  },
  "legend": { ... }
}
```

### Output JSON Schema (Annotated)

```json
{
  "nodes": [
    {
      "id": "0",
      "nodeType": "Source",
      "label": "source_iter",
      "fullLabel": "source_iter [iterate over collection]",
      "shortLabel": "source_iter",
      "data": {...},
      "analysis": {
        "nd_effect": "Deterministic",
        "monotone": true,
        "issues": []
      }
    }
  ],
  "edges": [
    {
      "id": "e0",
      "source": "0",
      "target": "1",
      "semanticTags": [...],
      "label": null,
      "analysis": {
        "is_lattice": false,
        "calm": "CalmSafe",
        "issues": []
      }
    }
  ],
  "overall": {
    "deterministic": true,
    "calm_safe": true
  },
  "hierarchyChoices": [...],
  "nodeAssignments": {...},
  "edgeStyleConfig": {...},
  "nodeTypeConfig": {...},
  "legend": {...}
}
```

## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system-essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

Based on the prework analysis, the following properties can be consolidated:

### Property 1: JSON Round-Trip Preservation
*For any* valid Hydro IR JSON, parsing then serializing (without analysis) should produce equivalent JSON structure.
**Validates: Requirements 1.1, 1.2, 1.3, 1.5, 7.6**

### Property 2: Malformed JSON Error Handling
*For any* malformed JSON input, Hydrolysis should report an error and not produce output.
**Validates: Requirements 1.4**

### Property 3: Operator Classification Completeness
*For any* node with a valid nodeType, the semantics lookup should return a valid NdEffect and Monotonicity value.
**Validates: Requirements 2.1, 2.2, 2.3**

### Property 4: Conservative Default for Unknown Types
*For any* node with an unknown nodeType, the semantics lookup should return conservative defaults (LocallyNonDet, Never).
**Validates: Requirements 2.5**

### Property 5: Lattice Detection Consistency
*For any* edge, the lattice detection should be deterministic based on the label content.
**Validates: Requirements 3.1, 3.2, 3.3**

### Property 6: ND Taint Propagation Transitivity
*For any* graph, if node A is tainted and there's a path from A to B, then B must also be tainted.
**Validates: Requirements 4.1, 4.2, 4.3**

### Property 7: Deterministic Nodes Have No ND Ancestors
*For any* node marked as Deterministic, there should be no path from any nondeterministic node to it.
**Validates: Requirements 4.4**

### Property 8: CALM Safety Path Verification
*For any* edge marked CalmSafe, all paths to that edge must use monotone operators and lattice types.
**Validates: Requirements 5.2, 5.3**

### Property 9: CALM Unsafe Detection
*For any* edge marked CalmUnsafe, there exists at least one path with a non-monotone operator or non-lattice edge.
**Validates: Requirements 5.4**

### Property 10: Overall CALM Consistency
*For any* analysis result, overall.calm_safe should be true if and only if all CALM-critical edges are CalmSafe.
**Validates: Requirements 5.5**

### Property 11: Issue Annotation Completeness
*For any* detected issue, it should appear in the analysis.issues array of the affected node or edge.
**Validates: Requirements 6.1, 6.2, 6.3, 6.4**

### Property 12: Output Structure Preservation
*For any* input JSON, the output should contain all original fields plus the analysis annotations.
**Validates: Requirements 7.1, 7.2, 7.3, 7.4**

## Error Handling

| Error Condition | Behavior |
|----------------|----------|
| Input file not found | Print error to stderr, exit with code 1 |
| Invalid JSON syntax | Print parse error with location, exit with code 1 |
| Missing required fields | Print field name and context, exit with code 1 |
| Output file write failure | Print error to stderr, exit with code 1 |
| Missing CLI arguments | Print usage information, exit with code 1 |

## Testing Strategy

### Dual Testing Approach

Both unit tests and property-based tests will be used:
- **Unit tests**: Verify specific examples, edge cases, and integration points
- **Property-based tests**: Verify universal properties across generated inputs

### Property-Based Testing Framework

Use `proptest` crate for Rust property-based testing. Configure each test to run minimum 100 iterations.

### Test Categories

1. **Parsing Tests**
   - Valid JSON parsing
   - Malformed JSON rejection
   - Missing field handling

2. **Semantics Tests**
   - Operator classification for all known types
   - Unknown type fallback behavior
   - Lattice type detection patterns

3. **Analysis Tests**
   - ND taint propagation on various graph topologies
   - CALM analysis on monotone/non-monotone paths
   - Issue generation for violations

4. **Output Tests**
   - JSON structure preservation
   - Annotation correctness
   - Pretty-print formatting

### Property Test Annotations

Each property-based test must be tagged with:
```rust
// **Feature: hydro-static-analysis, Property N: <property_text>**
// **Validates: Requirements X.Y**
```
