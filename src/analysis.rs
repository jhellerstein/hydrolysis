// Core analysis passes for ND taint propagation and CALM verification

use crate::model::{Edge, EdgeAnalysis, HydroIr, Node, NodeAnalysis, OverallAnalysis};
use std::collections::{HashMap, HashSet};

// String constants for analysis results
const ND_DETERMINISTIC: &str = "Deterministic";
const ND_LOCALLY_NONDET: &str = "LocallyNonDet";
const ND_EXTERNAL_NONDET: &str = "ExternalNonDet";
const CALM_SAFE: &str = "CalmSafe";
const CALM_UNSAFE: &str = "CalmUnsafe";

/// Combined analysis results
pub struct AnalysisResult {
    pub node_analyses: HashMap<String, NodeAnalysis>,
    pub edge_analyses: HashMap<String, EdgeAnalysis>,
    pub overall: OverallAnalysis,
}

/// Graph representation for analysis
struct Graph {
    /// Map from node ID to index
    node_id_to_idx: HashMap<String, usize>,
    /// Forward adjacency list: node index -> list of (target index, edge id)
    forward: Vec<Vec<(usize, String)>>,
    /// Backward adjacency list: node index -> list of (source index, edge id)
    backward: Vec<Vec<(usize, String)>>,
}

impl Graph {
    /// Build graph from nodes and edges
    fn build(nodes: &[Node], edges: &[Edge]) -> Self {
        let n = nodes.len();
        
        // Build node ID to index mapping
        let node_id_to_idx: HashMap<String, usize> = nodes
            .iter()
            .enumerate()
            .map(|(idx, node)| (node.id.clone(), idx))
            .collect();
        
        // Initialize adjacency lists
        let mut forward = vec![Vec::new(); n];
        let mut backward = vec![Vec::new(); n];
        
        // Build adjacency lists from edges
        for edge in edges {
            if let (Some(&src_idx), Some(&tgt_idx)) = (
                node_id_to_idx.get(&edge.source),
                node_id_to_idx.get(&edge.target),
            ) {
                forward[src_idx].push((tgt_idx, edge.id.clone()));
                backward[tgt_idx].push((src_idx, edge.id.clone()));
            }
        }
        
        Graph {
            node_id_to_idx,
            forward,
            backward,
        }
    }
    
    /// Get node index from ID
    fn get_idx(&self, node_id: &str) -> Option<usize> {
        self.node_id_to_idx.get(node_id).copied()
    }
}

/// ND taint propagation results
struct NdResults {
    /// Map from node ID to ND effect string
    nd_effects: HashMap<String, String>,
}

/// CALM analysis results
struct CalmResults {
    /// Map from edge ID to CALM status ("CalmSafe" or "CalmUnsafe")
    calm_status: HashMap<String, String>,
    /// Overall CALM safety
    overall_calm_safe: bool,
}

/// Run ND taint propagation pass
fn run_nd_pass(graph: &Graph, nodes: &[Node]) -> NdResults {
    use crate::semantics::{get_node_semantics, NdEffect};
    
    // Identify seed nodes (non-deterministic nodes)
    let mut tainted = HashSet::new();
    let mut nd_effects = HashMap::new();
    
    for (idx, node) in nodes.iter().enumerate() {
        let semantics = get_node_semantics(node);
        
        match semantics.nd {
            NdEffect::Deterministic => {
                // Not a seed, will be marked later if tainted
            }
            NdEffect::LocallyNonDet => {
                tainted.insert(idx);
                nd_effects.insert(node.id.clone(), ND_LOCALLY_NONDET.into());
            }
            NdEffect::ExternalNonDet => {
                tainted.insert(idx);
                nd_effects.insert(node.id.clone(), ND_EXTERNAL_NONDET.into());
            }
        }
    }
    
    // Compute transitive closure over outgoing edges
    let mut worklist: Vec<usize> = tainted.iter().copied().collect();
    
    while let Some(node_idx) = worklist.pop() {
        // For each successor of this tainted node
        for &(successor_idx, _) in &graph.forward[node_idx] {
            if !tainted.contains(&successor_idx) {
                // Mark successor as tainted
                tainted.insert(successor_idx);
                
                // Determine the ND effect for the successor
                // If it's tainted by propagation, use the original semantics or inherit
                let successor_id = &nodes[successor_idx].id;
                let semantics = get_node_semantics(&nodes[successor_idx]);
                
                let effect = match semantics.nd {
                    NdEffect::Deterministic => ND_LOCALLY_NONDET, // Tainted by propagation
                    NdEffect::LocallyNonDet => ND_LOCALLY_NONDET,
                    NdEffect::ExternalNonDet => ND_EXTERNAL_NONDET,
                };
                
                nd_effects.insert(successor_id.clone(), effect.into());
                worklist.push(successor_idx);
            }
        }
    }
    
    // Mark untainted nodes as Deterministic
    for node in nodes {
        nd_effects.entry(node.id.clone()).or_insert_with(|| ND_DETERMINISTIC.into());
    }
    
    NdResults { nd_effects }
}

/// Run CALM analysis pass
fn run_calm_pass(graph: &Graph, nodes: &[Node], edges: &[Edge]) -> CalmResults {
    let mut calm_status = HashMap::new();
    let mut all_calm_safe = true;
    
    // Identify CALM-critical edges
    let mut critical_edges = Vec::new();
    for edge in edges {
        let is_network = edge.semantic_tags.as_ref()
            .map(|tags| tags.iter().any(|tag| tag == "Network"))
            .unwrap_or(false);
        let targets_sink = graph.get_idx(&edge.target)
            .and_then(|idx| nodes.get(idx))
            .map(|node| node.node_type == "Sink")
            .unwrap_or(false);
        
        if is_network || targets_sink {
            critical_edges.push(edge);
        }
    }
    
    // For each critical edge, check all paths to it
    for edge in critical_edges {
        let edge_safe = check_edge_calm_safe(graph, nodes, edges, edge);
        
        let status = if edge_safe {
            CALM_SAFE
        } else {
            all_calm_safe = false;
            CALM_UNSAFE
        };
        
        calm_status.insert(edge.id.clone(), status.into());
    }
    
    // Mark non-critical edges as CalmSafe by default
    for edge in edges {
        calm_status.entry(edge.id.clone()).or_insert_with(|| CALM_SAFE.into());
    }
    
    CalmResults {
        calm_status,
        overall_calm_safe: all_calm_safe,
    }
}

/// Check if an edge is CALM safe by verifying all paths to it
fn check_edge_calm_safe(graph: &Graph, nodes: &[Node], edges: &[Edge], target_edge: &Edge) -> bool {
    use crate::semantics::{get_node_semantics, is_lattice_type, Monotonicity};
    
    // Get the target node index
    let target_idx = match graph.get_idx(&target_edge.target) {
        Some(idx) => idx,
        None => return true, // If target doesn't exist, consider safe
    };
    
    // Build edge lookup map
    let mut edge_map = HashMap::new();
    for edge in edges {
        if let (Some(src), Some(tgt)) = (graph.get_idx(&edge.source), graph.get_idx(&edge.target)) {
            edge_map.insert((src, tgt), edge);
        }
    }
    
    // Compute backward reachability from target node
    let reachable = compute_backward_reachable(graph, target_idx);
    
    // Check all paths: verify monotonicity and lattice types
    for &node_idx in &reachable {
        let node = &nodes[node_idx];
        let semantics = get_node_semantics(node);
        
        // Check if node is non-monotone
        if semantics.monotone == Monotonicity::Never {
            return false;
        }
        
        // Check outgoing edges from this node (that are on paths to target)
        for &(successor_idx, _) in &graph.forward[node_idx] {
            if reachable.contains(&successor_idx) || successor_idx == target_idx {
                // This edge is on a path to target
                if let Some(edge) = edge_map.get(&(node_idx, successor_idx))
                    && !is_lattice_type(edge.label.as_deref())
                {
                    return false;
                }
            }
        }
    }
    
    true
}

/// Compute backward reachable nodes from a target
fn compute_backward_reachable(graph: &Graph, target_idx: usize) -> HashSet<usize> {
    let mut reachable = HashSet::new();
    let mut worklist = vec![target_idx];
    reachable.insert(target_idx);
    
    while let Some(node_idx) = worklist.pop() {
        for &(predecessor_idx, _) in &graph.backward[node_idx] {
            if !reachable.contains(&predecessor_idx) {
                reachable.insert(predecessor_idx);
                worklist.push(predecessor_idx);
            }
        }
    }
    
    reachable
}

/// Extract issues from analysis results
fn extract_issues(
    ir: &HydroIr,
    graph: &Graph,
    nd_results: &NdResults,
    calm_results: &CalmResults,
    node_analyses: &mut HashMap<String, NodeAnalysis>,
    edge_analyses: &mut HashMap<String, EdgeAnalysis>,
) {
    use crate::model::Issue;
    use crate::semantics::{get_node_semantics, is_lattice_type, Monotonicity};
    
    // Generate NonDet issues for tainted nodes
    for node in &ir.nodes {
        if let Some(nd_effect) = nd_results.nd_effects.get(&node.id)
            && nd_effect != ND_DETERMINISTIC
            && let Some(analysis) = node_analyses.get_mut(&node.id)
        {
            analysis.issues.push(Issue {
                kind: "NonDet".to_string(),
                message: format!("Node '{}' is nondeterministic ({})", node.id, nd_effect),
            });
        }
    }
    
    // Build edge lookup map for CALM path checking
    let mut edge_map = HashMap::new();
    for edge in &ir.edges {
        if let (Some(src), Some(tgt)) = (graph.get_idx(&edge.source), graph.get_idx(&edge.target)) {
            edge_map.insert((src, tgt), edge);
        }
    }
    
    // Generate NonMonotone and NonLattice issues for CALM-critical edges
    for edge in &ir.edges {
        if let Some(calm_status) = calm_results.calm_status.get(&edge.id)
            && calm_status == CALM_UNSAFE
        {
            // This edge is CALM-unsafe, find the violations
            let target_idx = match graph.get_idx(&edge.target) {
                Some(idx) => idx,
                None => continue,
            };
            
            // Compute backward reachable nodes
            let reachable = compute_backward_reachable(graph, target_idx);
            
            // Check for non-monotone operators on paths
            for &node_idx in &reachable {
                let node = &ir.nodes[node_idx];
                let semantics = get_node_semantics(node);
                
                if semantics.monotone == Monotonicity::Never
                    && let Some(analysis) = node_analyses.get_mut(&node.id)
                {
                    analysis.issues.push(Issue {
                        kind: "NonMonotone".to_string(),
                        message: format!(
                            "Node '{}' is non-monotone on CALM-critical path to edge '{}'",
                            node.id, edge.id
                        ),
                    });
                }
                
                // Check outgoing edges for non-lattice types
                for &(successor_idx, _) in &graph.forward[node_idx] {
                    if (reachable.contains(&successor_idx) || successor_idx == target_idx)
                        && let Some(path_edge) = edge_map.get(&(node_idx, successor_idx))
                        && !is_lattice_type(path_edge.label.as_deref())
                        && let Some(analysis) = edge_analyses.get_mut(&path_edge.id)
                    {
                        analysis.issues.push(Issue {
                            kind: "NonLattice".to_string(),
                            message: format!(
                                "Edge '{}' is non-lattice on CALM-critical path to edge '{}'",
                                path_edge.id, edge.id
                            ),
                        });
                    }
                }
            }
        }
    }
}

/// Run all analysis passes on the input IR
pub fn run_analysis(ir: &HydroIr) -> AnalysisResult {
    // Build graph
    let graph = Graph::build(&ir.nodes, &ir.edges);
    
    // Run ND pass
    let nd_results = run_nd_pass(&graph, &ir.nodes);
    
    // Run CALM pass
    let calm_results = run_calm_pass(&graph, &ir.nodes, &ir.edges);
    
    // Compute overall deterministic status
    let overall_deterministic = nd_results.nd_effects.values()
        .all(|effect| effect == ND_DETERMINISTIC);
    
    // Create node analyses
    let mut node_analyses = HashMap::new();
    for node in &ir.nodes {
        let nd_effect = nd_results.nd_effects.get(&node.id)
            .cloned()
            .unwrap_or_else(|| ND_DETERMINISTIC.into());
        
        let semantics = crate::semantics::get_node_semantics(node);
        let monotone = semantics.monotone != crate::semantics::Monotonicity::Never;
        let source_location = node.extract_source_location();
        
        node_analyses.insert(node.id.clone(), NodeAnalysis {
            nd_effect,
            monotone,
            issues: Vec::new(),
            source_location,
        });
    }
    
    // Create edge analyses
    let mut edge_analyses = HashMap::new();
    for edge in &ir.edges {
        let is_lattice = crate::semantics::is_lattice_type(edge.label.as_deref());
        let calm = calm_results.calm_status.get(&edge.id)
            .cloned()
            .unwrap_or_else(|| CALM_SAFE.into());
        
        edge_analyses.insert(edge.id.clone(), EdgeAnalysis {
            is_lattice,
            calm,
            issues: Vec::new(),
        });
    }
    
    // Extract issues
    extract_issues(ir, &graph, &nd_results, &calm_results, &mut node_analyses, &mut edge_analyses);
    
    AnalysisResult {
        node_analyses,
        edge_analyses,
        overall: OverallAnalysis {
            deterministic: overall_deterministic,
            calm_safe: calm_results.overall_calm_safe,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{self, Node, Edge};
    use proptest::prelude::*;
    use std::collections::{HashMap, HashSet};

    // Use shared test helpers from model module
    use model::tests::{make_test_node, make_test_edge};

    #[test]
    fn test_deterministic_graph() {
        // Create a simple deterministic graph: Source -> Transform -> Sink
        let nodes = vec![
            make_test_node("0", "Source"),
            make_test_node("1", "Transform"),
            make_test_node("2", "Sink"),
        ];
        
        let edges = vec![
            make_test_edge("e0", "0", "1", vec!["Local"]),
            make_test_edge("e1", "1", "2", vec!["Local"]),
        ];
        
        let ir = HydroIr {
            nodes,
            edges,
            hierarchy_choices: None,
            node_assignments: None,
            selected_hierarchy: None,
            edge_style_config: None,
            node_type_config: None,
            legend: None,
        };
        
        let result = run_analysis(&ir);
        
        // All nodes should be deterministic
        assert_eq!(result.node_analyses.get("0").unwrap().nd_effect, "Deterministic");
        assert_eq!(result.node_analyses.get("1").unwrap().nd_effect, "Deterministic");
        assert_eq!(result.node_analyses.get("2").unwrap().nd_effect, "Deterministic");
        
        // Overall should be deterministic
        assert!(result.overall.deterministic);
        
        // No issues should be generated
        assert!(result.node_analyses.get("0").unwrap().issues.is_empty());
        assert!(result.node_analyses.get("1").unwrap().issues.is_empty());
        assert!(result.node_analyses.get("2").unwrap().issues.is_empty());
    }

    #[test]
    fn test_nondeterministic_propagation() {
        // Create a graph with ND propagation: Source -> NonDeterministic -> Transform -> Sink
        let nodes = vec![
            make_test_node("0", "Source"),
            make_test_node("1", "NonDeterministic"),
            make_test_node("2", "Transform"),
            make_test_node("3", "Sink"),
        ];
        
        let edges = vec![
            make_test_edge("e0", "0", "1", vec!["Local"]),
            make_test_edge("e1", "1", "2", vec!["Local"]),
            make_test_edge("e2", "2", "3", vec!["Local"]),
        ];
        
        let ir = HydroIr {
            nodes,
            edges,
            hierarchy_choices: None,
            node_assignments: None,
            selected_hierarchy: None,
            edge_style_config: None,
            node_type_config: None,
            legend: None,
        };
        
        let result = run_analysis(&ir);
        
        // Node 0 should be deterministic
        assert_eq!(result.node_analyses.get("0").unwrap().nd_effect, "Deterministic");
        
        // Node 1 should be LocallyNonDet (seed)
        assert_eq!(result.node_analyses.get("1").unwrap().nd_effect, "LocallyNonDet");
        
        // Nodes 2 and 3 should be tainted
        assert_eq!(result.node_analyses.get("2").unwrap().nd_effect, "LocallyNonDet");
        assert_eq!(result.node_analyses.get("3").unwrap().nd_effect, "LocallyNonDet");
        
        // Overall should be non-deterministic
        assert!(!result.overall.deterministic);
        
        // Issues should be generated for tainted nodes
        assert!(!result.node_analyses.get("1").unwrap().issues.is_empty());
        assert!(!result.node_analyses.get("2").unwrap().issues.is_empty());
        assert!(!result.node_analyses.get("3").unwrap().issues.is_empty());
    }

    #[test]
    fn test_calm_safe_network_edge() {
        // Create a graph with a Network edge that is CALM safe
        let nodes = vec![
            make_test_node("0", "Source"),
            make_test_node("1", "Transform"),
        ];
        
        let mut edges = vec![
            make_test_edge("e0", "0", "1", vec!["Network"]),
        ];
        // Add lattice type to make it CALM safe
        edges[0].label = Some("SetUnion<i32>".to_string());
        
        let ir = HydroIr {
            nodes,
            edges,
            hierarchy_choices: None,
            node_assignments: None,
            selected_hierarchy: None,
            edge_style_config: None,
            node_type_config: None,
            legend: None,
        };
        
        let result = run_analysis(&ir);
        
        // Edge should be CALM safe
        assert_eq!(result.edge_analyses.get("e0").unwrap().calm, "CalmSafe");
        assert!(result.edge_analyses.get("e0").unwrap().is_lattice);
        
        // Overall should be CALM safe
        assert!(result.overall.calm_safe);
    }

    // **Feature: hydro-static-analysis, Property 8: CALM Safety Path Verification**
    // **Validates: Requirements 5.2, 5.3**
    //
    // For any edge marked CalmSafe, all paths to that edge must use monotone operators and lattice types.

    // Strategy for generating graphs with CALM-critical edges
    fn arb_calm_graph() -> impl Strategy<Value = (Vec<Node>, Vec<Edge>, String)> {
        // Generate 2-8 nodes
        (2usize..8).prop_flat_map(|num_nodes| {
            // Pick which edge will be CALM-critical (Network edge or edge to Sink)
            let critical_edge_target = 1usize..num_nodes;
            
            critical_edge_target.prop_flat_map(move |target_idx| {
                // Generate node types - make target a Sink sometimes
                let make_sink = prop::bool::ANY;
                
                make_sink.prop_flat_map(move |is_sink| {
                    // Generate nodes with varying monotonicity
                    let node_types_strategy = prop::collection::vec(
                        prop::sample::select(vec![
                            "Source",      // Always monotone
                            "Transform",   // Always monotone
                            "Join",        // Always monotone
                            "Aggregation", // Depends (treated as monotone for this test)
                        ]),
                        num_nodes,
                    );
                    
                    node_types_strategy.prop_flat_map(move |mut node_types| {
                        // Set target node type
                        if is_sink {
                            node_types[target_idx] = "Sink";
                        }
                        
                        let nodes: Vec<Node> = node_types
                            .iter()
                            .enumerate()
                            .map(|(i, node_type)| make_test_node(&i.to_string(), node_type))
                            .collect();
                        
                        // Generate edges including a critical edge to target
                        // Strategy: create paths to the target node
                        let edge_strategy = prop::collection::vec(
                            (0usize..num_nodes)
                                .prop_filter("Create edges to target", move |src| {
                                    *src < target_idx
                                })
                                .prop_map(move |src| {
                                    let is_critical = src == target_idx - 1;
                                    let tags = if is_critical && !is_sink {
                                        vec!["Network"]
                                    } else {
                                        vec!["Local"]
                                    };
                                    
                                    let mut edge = make_test_edge(
                                        &format!("e_{}_{}", src, target_idx),
                                        &src.to_string(),
                                        &target_idx.to_string(),
                                        tags,
                                    );
                                    
                                    // Add lattice type to make it potentially CALM safe
                                    edge.label = Some("SetUnion<i32>".to_string());
                                    edge
                                }),
                            1..num_nodes, // At least one edge to target
                        );
                        
                        edge_strategy.prop_map(move |edges| {
                            let critical_edge_id = format!("e_{}_{}", target_idx - 1, target_idx);
                            (nodes.clone(), edges, critical_edge_id)
                        })
                    })
                })
            })
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn test_calm_safe_path_verification((nodes, edges, critical_edge_id) in arb_calm_graph()) {
            // Build the IR
            let ir = HydroIr {
                nodes: nodes.clone(),
                edges: edges.clone(),
                hierarchy_choices: None,
                node_assignments: None,
                selected_hierarchy: None,
                edge_style_config: None,
                node_type_config: None,
                legend: None,
            };
            
            // Run analysis
            let result = run_analysis(&ir);
            
            // Find the critical edge
            let critical_edge = edges.iter().find(|e| e.id == critical_edge_id);
            if critical_edge.is_none() {
                // Edge doesn't exist, skip this test case
                return Ok(());
            }
            let critical_edge = critical_edge.unwrap();
            
            // Get the CALM status of the critical edge
            let edge_analysis = result.edge_analyses.get(&critical_edge_id);
            if edge_analysis.is_none() {
                // No analysis for this edge, skip
                return Ok(());
            }
            let edge_analysis = edge_analysis.unwrap();
            
            // Build adjacency list for backward reachability
            let mut backward_adj: HashMap<String, Vec<String>> = HashMap::new();
            for edge in &edges {
                backward_adj
                    .entry(edge.target.clone())
                    .or_insert_with(Vec::new)
                    .push(edge.source.clone());
            }
            
            // Build edge lookup map
            let mut edge_map: HashMap<(String, String), &Edge> = HashMap::new();
            for edge in &edges {
                edge_map.insert((edge.source.clone(), edge.target.clone()), edge);
            }
            
            // Compute all nodes on paths to the critical edge target
            let mut nodes_on_paths = HashSet::new();
            let mut queue = vec![critical_edge.target.clone()];
            nodes_on_paths.insert(critical_edge.target.clone());
            
            while let Some(node_id) = queue.pop() {
                if let Some(predecessors) = backward_adj.get(&node_id) {
                    for pred in predecessors {
                        if !nodes_on_paths.contains(pred) {
                            nodes_on_paths.insert(pred.clone());
                            queue.push(pred.clone());
                        }
                    }
                }
            }
            
            // Property: If the edge is marked CalmSafe, verify all paths satisfy CALM requirements
            if edge_analysis.calm == "CalmSafe" {
                // Check all nodes on paths to the critical edge
                for node in &nodes {
                    if nodes_on_paths.contains(&node.id) {
                        let semantics = crate::semantics::get_semantics(&node.node_type);
                        
                        // Verify node is not explicitly non-monotone
                        prop_assert_ne!(
                            semantics.monotone,
                            crate::semantics::Monotonicity::Never,
                            "Edge {} is marked CalmSafe but node {} on path is non-monotone (type: {})",
                            critical_edge_id,
                            node.id,
                            node.node_type
                        );
                    }
                }
                
                // Check all edges on paths to the critical edge
                for edge in &edges {
                    let source_on_path = nodes_on_paths.contains(&edge.source);
                    let target_on_path = nodes_on_paths.contains(&edge.target);
                    
                    // If both source and target are on paths, this edge is on a path
                    if source_on_path && target_on_path {
                        // Verify edge has lattice type
                        let is_lattice = crate::semantics::is_lattice_type(edge.label.as_deref());
                        prop_assert!(
                            is_lattice,
                            "Edge {} is marked CalmSafe but edge {} on path is non-lattice (label: {:?})",
                            critical_edge_id,
                            edge.id,
                            edge.label
                        );
                    }
                }
            }
        }
    }

    // **Feature: hydro-static-analysis, Property 6: ND Taint Propagation Transitivity**
    // **Validates: Requirements 4.1, 4.2, 4.3**
    //
    // For any graph, if node A is tainted and there's a path from A to B, then B must also be tainted.

    // Strategy for generating a random graph with a nondeterministic seed
    fn arb_graph_with_nd_seed() -> impl Strategy<Value = (Vec<Node>, Vec<Edge>, usize)> {
        // Generate 3-10 nodes
        (3usize..10).prop_flat_map(|num_nodes| {
            // Pick which node will be the ND seed (not the first one to ensure paths exist)
            let nd_seed_idx = 1usize..num_nodes;
            
            nd_seed_idx.prop_flat_map(move |seed_idx| {
                // Generate nodes: one NonDeterministic at seed_idx, rest deterministic
                let nodes: Vec<Node> = (0..num_nodes)
                    .map(|i| {
                        let node_type = if i == seed_idx {
                            "NonDeterministic"
                        } else {
                            "Transform"
                        };
                        make_test_node(&i.to_string(), node_type)
                    })
                    .collect();
                
                // Generate edges to create paths
                // Strategy: create a DAG with some edges from lower to higher indices
                let edge_strategy = prop::collection::vec(
                    (0usize..num_nodes, 0usize..num_nodes)
                        .prop_filter("Create DAG edges", move |(src, tgt)| {
                            src < tgt && *tgt < num_nodes
                        })
                        .prop_map(|(src, tgt)| {
                            make_test_edge(
                                &format!("e_{}_{}", src, tgt),
                                &src.to_string(),
                                &tgt.to_string(),
                                vec!["Local"],
                            )
                        }),
                    0..num_nodes * 2, // Generate some edges
                );
                
                edge_strategy.prop_map(move |edges| (nodes.clone(), edges, seed_idx))
            })
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn test_nd_taint_transitivity((nodes, edges, seed_idx) in arb_graph_with_nd_seed()) {
            // Build the IR
            let ir = HydroIr {
                nodes: nodes.clone(),
                edges: edges.clone(),
                hierarchy_choices: None,
                node_assignments: None,
                selected_hierarchy: None,
                edge_style_config: None,
                node_type_config: None,
                legend: None,
            };
            
            // Run analysis
            let result = run_analysis(&ir);
            
            // Build adjacency list for reachability checking
            let mut adj: HashMap<String, Vec<String>> = HashMap::new();
            for edge in &edges {
                adj.entry(edge.source.clone())
                    .or_insert_with(Vec::new)
                    .push(edge.target.clone());
            }
            
            // Compute reachable nodes from the seed using BFS
            let seed_id = seed_idx.to_string();
            let mut reachable = HashSet::new();
            let mut queue = vec![seed_id.clone()];
            reachable.insert(seed_id.clone());
            
            while let Some(node_id) = queue.pop() {
                if let Some(neighbors) = adj.get(&node_id) {
                    for neighbor in neighbors {
                        if !reachable.contains(neighbor) {
                            reachable.insert(neighbor.clone());
                            queue.push(neighbor.clone());
                        }
                    }
                }
            }
            
            // Property: The seed node must be tainted
            let seed_analysis = result.node_analyses.get(&seed_id)
                .expect("Seed node should have analysis");
            prop_assert_ne!(
                &seed_analysis.nd_effect, 
                "Deterministic",
                "Seed node {} should be tainted", 
                seed_id
            );
            
            // Property: All reachable nodes from the seed must be tainted
            for node in &nodes {
                if reachable.contains(&node.id) {
                    let analysis = result.node_analyses.get(&node.id)
                        .expect("Node should have analysis");
                    prop_assert_ne!(
                        &analysis.nd_effect,
                        "Deterministic",
                        "Node {} is reachable from tainted node {} and should be tainted",
                        node.id,
                        seed_id
                    );
                }
            }
            
            // Property: Nodes not reachable from the seed should be deterministic
            for node in &nodes {
                if !reachable.contains(&node.id) {
                    let analysis = result.node_analyses.get(&node.id)
                        .expect("Node should have analysis");
                    prop_assert_eq!(
                        &analysis.nd_effect,
                        "Deterministic",
                        "Node {} is not reachable from tainted node {} and should be deterministic",
                        node.id,
                        seed_id
                    );
                }
            }
        }
    }

    // **Feature: hydro-static-analysis, Property 9: CALM Unsafe Detection**
    // **Validates: Requirements 5.4**
    //
    // For any edge marked CalmUnsafe, there exists at least one path with a non-monotone operator or non-lattice edge.

    // Strategy for generating graphs with CALM violations
    fn arb_calm_unsafe_graph() -> impl Strategy<Value = (Vec<Node>, Vec<Edge>, String)> {
        // Generate 2-8 nodes
        (2usize..8).prop_flat_map(|num_nodes| {
            // Pick which edge will be CALM-critical (Network edge or edge to Sink)
            let critical_edge_target = 1usize..num_nodes;
            
            critical_edge_target.prop_flat_map(move |target_idx| {
                // Generate node types - include some non-monotone nodes
                let node_types_strategy = prop::collection::vec(
                    prop::sample::select(vec![
                        "Source",      // Always monotone
                        "Transform",   // Always monotone
                        "Join",        // Always monotone
                        "NonDeterministic", // Non-monotone (Never)
                    ]),
                    num_nodes,
                );
                
                // Decide if we'll make the target a Sink (alternative to Network tag)
                let make_sink = prop::bool::ANY;
                
                // Decide if we'll introduce a non-lattice edge or non-monotone node
                let violation_type = prop::sample::select(vec!["non_monotone", "non_lattice"]);
                
                (node_types_strategy, make_sink, violation_type).prop_flat_map(move |(mut node_types, is_sink, violation_type)| {
                    // Set target node type
                    if is_sink {
                        node_types[target_idx] = "Sink";
                    }
                    
                    // If violation is non-monotone, ensure at least one node on path is non-monotone
                    if violation_type == "non_monotone" && target_idx > 0 {
                        // Make a node on the path non-monotone
                        let violation_node_idx = target_idx - 1;
                        node_types[violation_node_idx] = "NonDeterministic";
                    }
                    
                    let nodes: Vec<Node> = node_types
                        .iter()
                        .enumerate()
                        .map(|(i, node_type)| make_test_node(&i.to_string(), node_type))
                        .collect();
                    
                    // Generate edges including a critical edge to target
                    let edge_strategy = prop::collection::vec(
                        (0usize..num_nodes)
                            .prop_filter("Create edges to target", move |src| {
                                *src < target_idx
                            })
                            .prop_map(move |src| {
                                let is_critical = src == target_idx - 1;
                                let tags = if is_critical && !is_sink {
                                    vec!["Network"]
                                } else {
                                    vec!["Local"]
                                };
                                
                                let mut edge = make_test_edge(
                                    &format!("e_{}_{}", src, target_idx),
                                    &src.to_string(),
                                    &target_idx.to_string(),
                                    tags,
                                );
                                
                                // If violation is non-lattice, don't add lattice type
                                // Otherwise add lattice type
                                if violation_type == "non_lattice" && is_critical {
                                    edge.label = None; // Non-lattice
                                } else {
                                    edge.label = Some("SetUnion<i32>".to_string());
                                }
                                edge
                            }),
                        1..num_nodes, // At least one edge to target
                    );
                    
                    edge_strategy.prop_map(move |edges| {
                        let critical_edge_id = format!("e_{}_{}", target_idx - 1, target_idx);
                        (nodes.clone(), edges, critical_edge_id)
                    })
                })
            })
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn test_calm_unsafe_detection((nodes, edges, critical_edge_id) in arb_calm_unsafe_graph()) {
            // Build the IR
            let ir = HydroIr {
                nodes: nodes.clone(),
                edges: edges.clone(),
                hierarchy_choices: None,
                node_assignments: None,
                selected_hierarchy: None,
                edge_style_config: None,
                node_type_config: None,
                legend: None,
            };
            
            // Run analysis
            let result = run_analysis(&ir);
            
            // Find the critical edge
            let critical_edge = edges.iter().find(|e| e.id == critical_edge_id);
            if critical_edge.is_none() {
                // Edge doesn't exist, skip this test case
                return Ok(());
            }
            let critical_edge = critical_edge.unwrap();
            
            // Get the CALM status of the critical edge
            let edge_analysis = result.edge_analyses.get(&critical_edge_id);
            if edge_analysis.is_none() {
                // No analysis for this edge, skip
                return Ok(());
            }
            let edge_analysis = edge_analysis.unwrap();
            
            // Build adjacency list for backward reachability
            let mut backward_adj: HashMap<String, Vec<String>> = HashMap::new();
            for edge in &edges {
                backward_adj
                    .entry(edge.target.clone())
                    .or_insert_with(Vec::new)
                    .push(edge.source.clone());
            }
            
            // Build edge lookup map
            let mut edge_map: HashMap<(String, String), &Edge> = HashMap::new();
            for edge in &edges {
                edge_map.insert((edge.source.clone(), edge.target.clone()), edge);
            }
            
            // Compute all nodes on paths to the critical edge target
            let mut nodes_on_paths = HashSet::new();
            let mut queue = vec![critical_edge.target.clone()];
            nodes_on_paths.insert(critical_edge.target.clone());
            
            while let Some(node_id) = queue.pop() {
                if let Some(predecessors) = backward_adj.get(&node_id) {
                    for pred in predecessors {
                        if !nodes_on_paths.contains(pred) {
                            nodes_on_paths.insert(pred.clone());
                            queue.push(pred.clone());
                        }
                    }
                }
            }
            
            // Property: If the edge is marked CalmUnsafe, verify there's at least one violation
            if edge_analysis.calm == "CalmUnsafe" {
                let mut found_violation = false;
                
                // Check for non-monotone nodes on paths
                for node in &nodes {
                    if nodes_on_paths.contains(&node.id) {
                        let semantics = crate::semantics::get_semantics(&node.node_type);
                        
                        if semantics.monotone == crate::semantics::Monotonicity::Never {
                            found_violation = true;
                            break;
                        }
                    }
                }
                
                // Check for non-lattice edges on paths
                if !found_violation {
                    for edge in &edges {
                        let source_on_path = nodes_on_paths.contains(&edge.source);
                        let target_on_path = nodes_on_paths.contains(&edge.target);
                        
                        // If both source and target are on paths, this edge is on a path
                        if source_on_path && target_on_path {
                            let is_lattice = crate::semantics::is_lattice_type(edge.label.as_deref());
                            if !is_lattice {
                                found_violation = true;
                                break;
                            }
                        }
                    }
                }
                
                prop_assert!(
                    found_violation,
                    "Edge {} is marked CalmUnsafe but no violation (non-monotone node or non-lattice edge) found on paths to it",
                    critical_edge_id
                );
            }
        }
    }

    // **Feature: hydro-static-analysis, Property 10: Overall CALM Consistency**
    // **Validates: Requirements 5.5**
    //
    // For any analysis result, overall.calm_safe should be true if and only if all CALM-critical edges are CalmSafe.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn test_overall_calm_consistency((nodes, edges, _critical_edge_id) in arb_calm_graph()) {
            // Build the IR
            let ir = HydroIr {
                nodes: nodes.clone(),
                edges: edges.clone(),
                hierarchy_choices: None,
                node_assignments: None,
                selected_hierarchy: None,
                edge_style_config: None,
                node_type_config: None,
                legend: None,
            };
            
            // Run analysis
            let result = run_analysis(&ir);
            
            // Identify all CALM-critical edges (Network edges or edges to Sink nodes)
            let mut critical_edges = Vec::new();
            for edge in &edges {
                let is_network = edge.semantic_tags.as_ref()
                    .map(|tags| tags.iter().any(|tag| tag == "Network"))
                    .unwrap_or(false);
                let targets_sink = nodes.iter()
                    .find(|n| n.id == edge.target)
                    .map(|n| n.node_type == "Sink")
                    .unwrap_or(false);
                
                if is_network || targets_sink {
                    critical_edges.push(&edge.id);
                }
            }
            
            // Check if all critical edges are CalmSafe
            let all_critical_edges_safe = critical_edges.iter().all(|edge_id| {
                result.edge_analyses.get(*edge_id)
                    .map(|analysis| analysis.calm == "CalmSafe")
                    .unwrap_or(false)
            });
            
            // Property: overall.calm_safe should be true iff all critical edges are CalmSafe
            prop_assert_eq!(
                result.overall.calm_safe,
                all_critical_edges_safe,
                "overall.calm_safe ({}) does not match whether all critical edges are CalmSafe ({}). Critical edges: {:?}",
                result.overall.calm_safe,
                all_critical_edges_safe,
                critical_edges.iter().map(|id| {
                    let analysis = result.edge_analyses.get(*id);
                    (id, analysis.map(|a| a.calm.as_str()))
                }).collect::<Vec<_>>()
            );
            
            // Additional verification: if overall.calm_safe is true, no critical edge should be CalmUnsafe
            if result.overall.calm_safe {
                for edge_id in &critical_edges {
                    let edge_analysis = result.edge_analyses.get(*edge_id)
                        .expect("Critical edge should have analysis");
                    prop_assert_eq!(
                        &edge_analysis.calm,
                        "CalmSafe",
                        "overall.calm_safe is true but critical edge {} is {}",
                        edge_id,
                        edge_analysis.calm
                    );
                }
            }
            
            // Additional verification: if overall.calm_safe is false, at least one critical edge should be CalmUnsafe
            if !result.overall.calm_safe {
                let has_unsafe_edge = critical_edges.iter().any(|edge_id| {
                    result.edge_analyses.get(*edge_id)
                        .map(|analysis| analysis.calm == "CalmUnsafe")
                        .unwrap_or(false)
                });
                
                prop_assert!(
                    has_unsafe_edge,
                    "overall.calm_safe is false but no critical edge is CalmUnsafe. Critical edges: {:?}",
                    critical_edges.iter().map(|id| {
                        let analysis = result.edge_analyses.get(*id);
                        (id, analysis.map(|a| a.calm.as_str()))
                    }).collect::<Vec<_>>()
                );
            }
        }
    }

    // **Feature: hydro-static-analysis, Property 11: Issue Annotation Completeness**
    // **Validates: Requirements 6.1, 6.2, 6.3, 6.4**
    //
    // For any detected issue, it should appear in the analysis.issues array of the affected node or edge.

    // Strategy for generating graphs with various issue types
    fn arb_graph_with_issues() -> impl Strategy<Value = (Vec<Node>, Vec<Edge>)> {
        // Generate 3-10 nodes
        (3usize..10).prop_flat_map(|num_nodes| {
            // Generate node types including some that will cause issues
            let node_types_strategy = prop::collection::vec(
                prop::sample::select(vec![
                    "Source",           // Deterministic, monotone
                    "Transform",        // Deterministic, monotone
                    "NonDeterministic", // NonDet issue
                    "Join",             // Deterministic, monotone
                ]),
                num_nodes,
            );
            
            node_types_strategy.prop_flat_map(move |node_types| {
                let nodes: Vec<Node> = node_types
                    .iter()
                    .enumerate()
                    .map(|(i, node_type)| make_test_node(&i.to_string(), node_type))
                    .collect();
                
                // Generate edges including some Network edges and edges to Sinks
                // Some edges will have lattice types, some won't (to trigger NonLattice issues)
                let edge_strategy = prop::collection::vec(
                    (0usize..num_nodes, 0usize..num_nodes, prop::bool::ANY, prop::bool::ANY)
                        .prop_filter("Create valid edges", move |(src, tgt, _, _)| {
                            src < tgt && *tgt < num_nodes
                        })
                        .prop_map(move |(src, tgt, is_network, has_lattice)| {
                            let tags = if is_network {
                                vec!["Network"]
                            } else {
                                vec!["Local"]
                            };
                            
                            let mut edge = make_test_edge(
                                &format!("e_{}_{}", src, tgt),
                                &src.to_string(),
                                &tgt.to_string(),
                                tags,
                            );
                            
                            // Add lattice type conditionally
                            if has_lattice {
                                edge.label = Some("SetUnion<i32>".to_string());
                            } else {
                                edge.label = None;
                            }
                            edge
                        }),
                    1..num_nodes * 2,
                );
                
                edge_strategy.prop_map(move |edges| (nodes.clone(), edges))
            })
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn test_issue_annotation_completeness((nodes, edges) in arb_graph_with_issues()) {
            // Build the IR
            let ir = HydroIr {
                nodes: nodes.clone(),
                edges: edges.clone(),
                hierarchy_choices: None,
                node_assignments: None,
                selected_hierarchy: None,
                edge_style_config: None,
                node_type_config: None,
                legend: None,
            };
            
            // Run analysis
            let result = run_analysis(&ir);
            
            // Property 1: Every nondeterministic node should have a NonDet issue
            for node in &nodes {
                let analysis = result.node_analyses.get(&node.id)
                    .expect("Node should have analysis");
                
                if analysis.nd_effect != "Deterministic" {
                    // This node is nondeterministic, it should have a NonDet issue
                    let has_nondet_issue = analysis.issues.iter()
                        .any(|issue| issue.kind == "NonDet");
                    
                    prop_assert!(
                        has_nondet_issue,
                        "Node {} has nd_effect '{}' but no NonDet issue in its issues array",
                        node.id,
                        analysis.nd_effect
                    );
                }
            }
            
            // Property 2: Every NonDet issue should correspond to a nondeterministic node
            for node in &nodes {
                let analysis = result.node_analyses.get(&node.id)
                    .expect("Node should have analysis");
                
                for issue in &analysis.issues {
                    if issue.kind == "NonDet" {
                        prop_assert_ne!(
                            &analysis.nd_effect,
                            "Deterministic",
                            "Node {} has a NonDet issue but is marked as Deterministic",
                            node.id
                        );
                    }
                }
            }
            
            // Property 3: Every NonMonotone issue should correspond to a non-monotone node on a CALM path
            for node in &nodes {
                let analysis = result.node_analyses.get(&node.id)
                    .expect("Node should have analysis");
                
                for issue in &analysis.issues {
                    if issue.kind == "NonMonotone" {
                        // Verify the node is actually non-monotone
                        let semantics = crate::semantics::get_semantics(&node.node_type);
                        prop_assert_eq!(
                            semantics.monotone,
                            crate::semantics::Monotonicity::Never,
                            "Node {} has a NonMonotone issue but is not non-monotone (type: {})",
                            node.id,
                            node.node_type
                        );
                        
                        // Verify there's at least one CALM-unsafe edge that this node affects
                        let has_calm_unsafe_edge = result.edge_analyses.values()
                            .any(|edge_analysis| edge_analysis.calm == "CalmUnsafe");
                        
                        prop_assert!(
                            has_calm_unsafe_edge,
                            "Node {} has a NonMonotone issue but no CALM-unsafe edges exist",
                            node.id
                        );
                    }
                }
            }
            
            // Property 4: Every NonLattice issue should correspond to a non-lattice edge on a CALM path
            for edge in &edges {
                let analysis = result.edge_analyses.get(&edge.id)
                    .expect("Edge should have analysis");
                
                for issue in &analysis.issues {
                    if issue.kind == "NonLattice" {
                        // Verify the edge is actually non-lattice
                        prop_assert!(
                            !analysis.is_lattice,
                            "Edge {} has a NonLattice issue but is marked as lattice",
                            edge.id
                        );
                        
                        // Verify there's at least one CALM-unsafe edge
                        let has_calm_unsafe_edge = result.edge_analyses.values()
                            .any(|edge_analysis| edge_analysis.calm == "CalmUnsafe");
                        
                        prop_assert!(
                            has_calm_unsafe_edge,
                            "Edge {} has a NonLattice issue but no CALM-unsafe edges exist",
                            edge.id
                        );
                    }
                }
            }
            
            // Property 5: If a CALM-critical edge is CalmUnsafe, there should be issues on the path
            for edge in &edges {
                let is_network = edge.semantic_tags.as_ref()
                    .map(|tags| tags.iter().any(|tag| tag == "Network"))
                    .unwrap_or(false);
                let targets_sink = nodes.iter()
                    .find(|n| n.id == edge.target)
                    .map(|n| n.node_type == "Sink")
                    .unwrap_or(false);
                
                if is_network || targets_sink {
                    // This is a CALM-critical edge
                    let edge_analysis = result.edge_analyses.get(&edge.id)
                        .expect("Critical edge should have analysis");
                    
                    if edge_analysis.calm == "CalmUnsafe" {
                        // There should be at least one issue somewhere (NonMonotone or NonLattice)
                        let has_node_issues = result.node_analyses.values()
                            .any(|node_analysis| {
                                node_analysis.issues.iter()
                                    .any(|issue| issue.kind == "NonMonotone")
                            });
                        
                        let has_edge_issues = result.edge_analyses.values()
                            .any(|edge_analysis| {
                                edge_analysis.issues.iter()
                                    .any(|issue| issue.kind == "NonLattice")
                            });
                        
                        prop_assert!(
                            has_node_issues || has_edge_issues,
                            "Edge {} is CalmUnsafe but no NonMonotone or NonLattice issues were generated",
                            edge.id
                        );
                    }
                }
            }
        }
    }

    // **Feature: hydro-static-analysis, Property 7: Deterministic Nodes Have No ND Ancestors**
    // **Validates: Requirements 4.4**
    //
    // For any node marked as Deterministic, there should be no path from any nondeterministic node to it.
    // This is the inverse property of Property 6 - it verifies correctness from the deterministic nodes' perspective.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn test_deterministic_nodes_have_no_nd_ancestors((nodes, edges, _seed_idx) in arb_graph_with_nd_seed()) {
            // Build the IR
            let ir = HydroIr {
                nodes: nodes.clone(),
                edges: edges.clone(),
                hierarchy_choices: None,
                node_assignments: None,
                selected_hierarchy: None,
                edge_style_config: None,
                node_type_config: None,
                legend: None,
            };
            
            // Run analysis
            let result = run_analysis(&ir);
            
            // Build adjacency list for forward reachability from ND nodes
            let mut adj: HashMap<String, Vec<String>> = HashMap::new();
            for edge in &edges {
                adj.entry(edge.source.clone())
                    .or_insert_with(Vec::new)
                    .push(edge.target.clone());
            }
            
            // Identify all nondeterministic nodes (seeds)
            let mut nd_nodes = HashSet::new();
            for node in &nodes {
                // A node is a seed if it's inherently nondeterministic (not just tainted)
                if node.node_type == "NonDeterministic" {
                    nd_nodes.insert(node.id.clone());
                }
            }
            
            // For each ND seed, compute all reachable nodes
            let mut reachable_from_nd = HashSet::new();
            for nd_node_id in &nd_nodes {
                let mut queue = vec![nd_node_id.clone()];
                let mut visited = HashSet::new();
                visited.insert(nd_node_id.clone());
                
                while let Some(node_id) = queue.pop() {
                    reachable_from_nd.insert(node_id.clone());
                    
                    if let Some(neighbors) = adj.get(&node_id) {
                        for neighbor in neighbors {
                            if !visited.contains(neighbor) {
                                visited.insert(neighbor.clone());
                                queue.push(neighbor.clone());
                            }
                        }
                    }
                }
            }
            
            // Property: For each node marked as Deterministic, verify it's not reachable from any ND node
            for node in &nodes {
                let analysis = result.node_analyses.get(&node.id)
                    .expect("Node should have analysis");
                
                if analysis.nd_effect == "Deterministic" {
                    prop_assert!(
                        !reachable_from_nd.contains(&node.id),
                        "Node {} is marked Deterministic but is reachable from a nondeterministic node",
                        node.id
                    );
                }
            }
            
            // Inverse property: For each node reachable from an ND node, it should NOT be marked Deterministic
            for node in &nodes {
                if reachable_from_nd.contains(&node.id) {
                    let analysis = result.node_analyses.get(&node.id)
                        .expect("Node should have analysis");
                    
                    prop_assert_ne!(
                        &analysis.nd_effect,
                        "Deterministic",
                        "Node {} is reachable from a nondeterministic node but is marked Deterministic",
                        node.id
                    );
                }
            }
        }
    }
}
