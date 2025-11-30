// Merge analysis results into annotated JSON output

use crate::analysis::AnalysisResult;
use crate::model::{AnnotatedEdge, AnnotatedHydroIr, AnnotatedNode, HydroIr};
use anyhow::Result;

/// Merge analysis results into the original IR structure
pub fn annotate(ir: &HydroIr, results: &AnalysisResult) -> AnnotatedHydroIr {
    use crate::semantics::{get_node_semantics, NdEffect};
    
    // Convert nodes to annotated nodes with semantic tags
    let annotated_nodes: Vec<AnnotatedNode> = ir
        .nodes
        .iter()
        .map(|node| {
            let analysis = results.node_analyses.get(&node.id);
            
            // Keep original node type but add semantic tags for styling
            let mut semantic_tags = vec![node.node_type.clone()];
            
            if let Some(analysis) = analysis {
                let semantics = get_node_semantics(node);
                let is_root_cause = semantics.nd != NdEffect::Deterministic;
                
                if is_root_cause {
                    // This node is a root cause of nondeterminism
                    semantic_tags.push("NonDetRoot".to_string());
                } else if analysis.nd_effect != "Deterministic" {
                    // This node inherits nondeterminism
                    semantic_tags.push("NonDetInherited".to_string());
                } else {
                    // Deterministic node
                    semantic_tags.push("Deterministic".to_string());
                }
                
                // Add monotonicity tag
                if analysis.monotone {
                    semantic_tags.push("Monotone".to_string());
                } else {
                    semantic_tags.push("NonMonotone".to_string());
                }
            }
            
            AnnotatedNode {
                id: node.id.clone(),
                node_type: node.node_type.clone(),
                semantic_tags: Some(semantic_tags),
                short_label: node.short_label.clone(),
                full_label: node.full_label.clone(),
                label: node.label.clone(),
                data: node.data.clone(),
                analysis: analysis.cloned(),
            }
        })
        .collect();

    // Convert edges to annotated edges with enhanced semantic tags
    let annotated_edges: Vec<AnnotatedEdge> = ir
        .edges
        .iter()
        .map(|edge| {
            let analysis = results.edge_analyses.get(&edge.id);
            
            // Add analysis-based semantic tags
            let mut enhanced_tags = edge.semantic_tags.clone().unwrap_or_default();
            
            if let Some(analysis) = analysis {
                // Add lattice/non-lattice tag
                if analysis.is_lattice {
                    enhanced_tags.push("Lattice".to_string());
                } else {
                    enhanced_tags.push("NonLattice".to_string());
                }
                
                // Add CALM status tag
                enhanced_tags.push(analysis.calm.clone());
            }
            
            AnnotatedEdge {
                id: edge.id.clone(),
                source: edge.source.clone(),
                target: edge.target.clone(),
                edge_properties: edge.edge_properties.clone(),
                semantic_tags: Some(enhanced_tags),
                label: edge.label.clone(),
                analysis: analysis.cloned(),
            }
        })
        .collect();

    // Create enhanced configs for visualization
    let enhanced_node_config = create_enhanced_node_config(ir.node_type_config.as_ref());
    let enhanced_edge_config = create_enhanced_edge_config(ir.edge_style_config.as_ref());
    
    // Create annotated IR with enhanced configs
    AnnotatedHydroIr {
        nodes: annotated_nodes,
        edges: annotated_edges,
        overall: Some(results.overall.clone()),
        hierarchy_choices: ir.hierarchy_choices.clone(),
        node_assignments: ir.node_assignments.clone(),
        selected_hierarchy: ir.selected_hierarchy.clone(),
        edge_style_config: Some(enhanced_edge_config),
        node_type_config: Some(enhanced_node_config),
        legend: ir.legend.clone(),
    }
}

/// Create enhanced node type config with analysis-specific semantic mappings
fn create_enhanced_node_config(original: Option<&serde_json::Value>) -> serde_json::Value {
    let mut config = original.cloned().unwrap_or_else(|| {
        serde_json::json!({
            "defaultType": "Transform",
            "types": [],
            "semanticMappings": {}
        })
    });
    
    // Don't override node type colors - let Hydroscope handle that based on nodeType
    // We only add semantic mappings for analysis-specific styling
    
    // Add analysis-specific semantic mappings for node styling
    let analysis_mappings = serde_json::json!({
        "NondeterminismGroup": {
            "NonDetRoot": {
                "color-token": "warning",
                "border-width": 3
            },
            "NonDetInherited": {
                "color-token": "warning-light",
                "border-style": "dashed"
            },
            "Deterministic": {
                "color-token": "default"
            }
        },
        "MonotonicityGroup": {
            "Monotone": {
                "badge": "✓"
            },
            "NonMonotone": {
                "badge": "⚠"
            }
        }
    });
    
    if let Some(mappings) = config.get_mut("semanticMappings").and_then(|m| m.as_object_mut()) {
        for (key, value) in analysis_mappings.as_object().unwrap() {
            mappings.insert(key.clone(), value.clone());
        }
    } else {
        config.as_object_mut().unwrap().insert(
            "semanticMappings".to_string(),
            analysis_mappings
        );
    }
    
    config
}

/// Create enhanced edge style config with analysis-specific semantic mappings
fn create_enhanced_edge_config(original: Option<&serde_json::Value>) -> serde_json::Value {
    let mut config = original.cloned().unwrap_or_else(|| {
        serde_json::json!({"semanticMappings": {}})
    });
    
    // Add analysis-specific semantic mappings with distinct visual styles
    let analysis_mappings = serde_json::json!({
        "LatticeGroup": {
            "Lattice": {
                "color-token": "success"
            },
            "NonLattice": {
                "color-token": "danger"
            }
        },
        "CALMGroup": {
            "CalmSafe": {
                "line-pattern": "solid",
                "line-width": 2
            },
            "CalmUnsafe": {
                "line-pattern": "dashed",
                "line-width": 3
            }
        }
    });
    
    if let Some(mappings) = config.get_mut("semanticMappings").and_then(|m| m.as_object_mut()) {
        for (key, value) in analysis_mappings.as_object().unwrap() {
            mappings.insert(key.clone(), value.clone());
        }
    }
    
    config
}

/// Annotate the input IR with analysis results and serialize to JSON
pub fn annotate_and_serialize(ir: &HydroIr, results: &AnalysisResult) -> Result<String> {
    let annotated = annotate(ir, results);
    let json = serde_json::to_string_pretty(&annotated)?;
    Ok(json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::run_analysis;
    use crate::model::{self, Edge, HydroIr, Node, NodeData};
    use proptest::prelude::*;

    // **Feature: hydro-static-analysis, Property 12: Output Structure Preservation**
    // **Validates: Requirements 7.1, 7.2, 7.3, 7.4**
    //
    // For any input JSON, the output should contain all original fields plus the analysis annotations.

    // Use shared test helpers from model module
    use model::tests::{make_test_node, make_test_edge};

    // Strategy for generating valid NodeData
    fn arb_node_data() -> impl Strategy<Value = NodeData> {
        (
            prop::option::of(any::<usize>()),
            prop::option::of(prop::string::string_regex("[A-Za-z]+").unwrap()),
            prop::collection::vec(any::<i32>(), 0..3),
        )
            .prop_map(|(location_id, location_type, backtrace_data)| NodeData {
                location_id,
                location_type,
                backtrace: serde_json::json!(backtrace_data),
            })
    }

    // Strategy for generating valid Nodes
    fn arb_node() -> impl Strategy<Value = Node> {
        (
            prop::string::string_regex("[0-9]+").unwrap(),
            prop::sample::select(vec![
                "Source".to_string(),
                "Transform".to_string(),
                "Join".to_string(),
                "Aggregation".to_string(),
                "Network".to_string(),
                "Sink".to_string(),
                "Tee".to_string(),
                "NonDeterministic".to_string(),
            ]),
            prop::string::string_regex("[a-z_]+").unwrap(),
            prop::option::of(prop::string::string_regex("[a-z_ \\[\\]]+").unwrap()),
            prop::option::of(prop::string::string_regex("[a-z_]+").unwrap()),
            prop::option::of(arb_node_data()),
        )
            .prop_map(
                |(id, node_type, short_label, full_label, label, data)| Node {
                    id,
                    node_type,
                    short_label,
                    full_label,
                    label,
                    data,
                },
            )
    }

    // Strategy for generating valid Edges
    fn arb_edge() -> impl Strategy<Value = Edge> {
        (
            prop::string::string_regex("e[0-9]+").unwrap(),
            prop::string::string_regex("[0-9]+").unwrap(),
            prop::string::string_regex("[0-9]+").unwrap(),
            prop::option::of(prop::collection::vec(
                prop::sample::select(vec![
                    "Local".to_string(),
                    "Network".to_string(),
                    "Stream".to_string(),
                    "Bounded".to_string(),
                    "Unbounded".to_string(),
                    "TotalOrder".to_string(),
                    "NoOrder".to_string(),
                    "Keyed".to_string(),
                ]),
                1..4,
            )),
            prop::option::of(prop::collection::vec(
                prop::sample::select(vec![
                    "Local".to_string(),
                    "Network".to_string(),
                    "Stream".to_string(),
                    "Bounded".to_string(),
                    "Unbounded".to_string(),
                    "TotalOrder".to_string(),
                    "NoOrder".to_string(),
                    "Keyed".to_string(),
                ]),
                1..4,
            )),
            prop::option::of(prop::string::string_regex("[A-Za-z<>:, ]+").unwrap()),
        )
            .prop_map(|(id, source, target, edge_properties, semantic_tags, label)| Edge {
                id,
                source,
                target,
                edge_properties,
                semantic_tags,
                label,
            })
    }

    // Strategy for generating valid HydroIr
    fn arb_hydro_ir() -> impl Strategy<Value = HydroIr> {
        (
            prop::collection::vec(arb_node(), 1..10),
            prop::collection::vec(arb_edge(), 0..15),
            prop::option::of(Just(serde_json::json!([{"id": "location", "name": "Location"}]))),
            prop::option::of(Just(serde_json::json!({"location": {"0": "loc_0"}}))),
            prop::option::of(Just("location".to_string())),
            prop::option::of(Just(serde_json::json!({"default": "solid"}))),
            prop::option::of(Just(serde_json::json!({"defaultType": "Transform"}))),
            prop::option::of(Just(serde_json::json!({"show": true}))),
        )
            .prop_map(
                |(
                    nodes,
                    edges,
                    hierarchy_choices,
                    node_assignments,
                    selected_hierarchy,
                    edge_style_config,
                    node_type_config,
                    legend,
                )| HydroIr {
                    nodes,
                    edges,
                    hierarchy_choices,
                    node_assignments,
                    selected_hierarchy,
                    edge_style_config,
                    node_type_config,
                    legend,
                },
            )
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn test_output_structure_preservation(hydro_ir in arb_hydro_ir()) {
            // Run analysis
            let results = run_analysis(&hydro_ir);

            // Annotate the IR
            let annotated = annotate(&hydro_ir, &results);

            // Property 1: All original nodes should be present
            prop_assert_eq!(
                annotated.nodes.len(),
                hydro_ir.nodes.len(),
                "Annotated output should have same number of nodes as input"
            );

            // Property 2: All original edges should be present
            prop_assert_eq!(
                annotated.edges.len(),
                hydro_ir.edges.len(),
                "Annotated output should have same number of edges as input"
            );

            // Property 3: All node fields should be preserved
            for (original, annotated_node) in hydro_ir.nodes.iter().zip(annotated.nodes.iter()) {
                prop_assert_eq!(
                    &annotated_node.id,
                    &original.id,
                    "Node ID should be preserved"
                );
                prop_assert_eq!(
                    &annotated_node.node_type,
                    &original.node_type,
                    "Node type should be preserved"
                );
                prop_assert_eq!(
                    &annotated_node.label,
                    &original.label,
                    "Node label should be preserved"
                );
                prop_assert_eq!(
                    &annotated_node.full_label,
                    &original.full_label,
                    "Node full_label should be preserved"
                );
                prop_assert_eq!(
                    &annotated_node.short_label,
                    &original.short_label,
                    "Node short_label should be preserved"
                );
                prop_assert_eq!(
                    annotated_node.data.as_ref().and_then(|d| d.location_id),
                    original.data.as_ref().and_then(|d| d.location_id),
                    "Node data.location_id should be preserved"
                );
                prop_assert_eq!(
                    annotated_node.data.as_ref().and_then(|d| d.location_type.as_ref()),
                    original.data.as_ref().and_then(|d| d.location_type.as_ref()),
                    "Node data.location_type should be preserved"
                );

                // Property 4: Analysis should be added to each node
                prop_assert!(
                    annotated_node.analysis.is_some(),
                    "Node {} should have analysis annotation",
                    annotated_node.id
                );

                // Verify analysis fields are present
                if let Some(analysis) = &annotated_node.analysis {
                    prop_assert!(
                        !analysis.nd_effect.is_empty(),
                        "Node {} analysis should have nd_effect",
                        annotated_node.id
                    );
                    // monotone is a bool, always present
                    // issues is a Vec, always present (may be empty)
                }
            }

            // Property 5: All edge fields should be preserved (semantic_tags are enhanced)
            for (original, annotated_edge) in hydro_ir.edges.iter().zip(annotated.edges.iter()) {
                prop_assert_eq!(
                    &annotated_edge.id,
                    &original.id,
                    "Edge ID should be preserved"
                );
                prop_assert_eq!(
                    &annotated_edge.source,
                    &original.source,
                    "Edge source should be preserved"
                );
                prop_assert_eq!(
                    &annotated_edge.target,
                    &original.target,
                    "Edge target should be preserved"
                );
                // semantic_tags are enhanced with analysis tags, so just check they exist
                prop_assert!(
                    annotated_edge.semantic_tags.is_some(),
                    "Edge semantic_tags should be present (enhanced)"
                );
                // Original tags should be included in enhanced tags
                if let (Some(original_tags), Some(annotated_tags)) = (&original.semantic_tags, &annotated_edge.semantic_tags) {
                    for tag in original_tags {
                        prop_assert!(
                            annotated_tags.contains(tag),
                            "Original tag '{}' should be preserved in enhanced tags",
                            tag
                        );
                    }
                }
                prop_assert_eq!(
                    &annotated_edge.label,
                    &original.label,
                    "Edge label should be preserved"
                );

                // Property 6: Analysis should be added to each edge
                prop_assert!(
                    annotated_edge.analysis.is_some(),
                    "Edge {} should have analysis annotation",
                    annotated_edge.id
                );

                // Verify analysis fields are present
                if let Some(analysis) = &annotated_edge.analysis {
                    // is_lattice is a bool, always present
                    prop_assert!(
                        !analysis.calm.is_empty(),
                        "Edge {} analysis should have calm status",
                        annotated_edge.id
                    );
                    // issues is a Vec, always present (may be empty)
                }
            }

            // Property 7: Overall analysis should be present
            prop_assert!(
                annotated.overall.is_some(),
                "Annotated output should have overall analysis"
            );

            // Property 8: All optional original fields should be preserved or enhanced
            prop_assert_eq!(
                annotated.hierarchy_choices,
                hydro_ir.hierarchy_choices,
                "hierarchy_choices should be preserved"
            );
            prop_assert_eq!(
                annotated.node_assignments,
                hydro_ir.node_assignments,
                "node_assignments should be preserved"
            );
            prop_assert_eq!(
                annotated.selected_hierarchy,
                hydro_ir.selected_hierarchy,
                "selected_hierarchy should be preserved"
            );
            // edge_style_config and node_type_config are enhanced, so they're always Some
            prop_assert!(
                annotated.edge_style_config.is_some(),
                "edge_style_config should be present (enhanced)"
            );
            prop_assert!(
                annotated.node_type_config.is_some(),
                "node_type_config should be present (enhanced)"
            );
            prop_assert_eq!(
                annotated.legend,
                hydro_ir.legend,
                "legend should be preserved"
            );
        }
    }

    #[test]
    fn test_simple_annotation() {
        // Create a simple test case
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
            hierarchy_choices: Some(serde_json::json!([{"id": "location"}])),
            node_assignments: Some(serde_json::json!({"location": {"0": "loc_0"}})),
            selected_hierarchy: Some("location".to_string()),
            edge_style_config: None,
            node_type_config: None,
            legend: None,
        };

        // Run analysis
        let results = run_analysis(&ir);

        // Annotate
        let annotated = annotate(&ir, &results);

        // Verify structure
        assert_eq!(annotated.nodes.len(), 3);
        assert_eq!(annotated.edges.len(), 2);
        assert!(annotated.overall.is_some());

        // Verify all nodes have analysis
        for node in &annotated.nodes {
            assert!(node.analysis.is_some());
        }

        // Verify all edges have analysis
        for edge in &annotated.edges {
            assert!(edge.analysis.is_some());
        }

        // Verify optional fields preserved
        assert!(annotated.hierarchy_choices.is_some());
        assert!(annotated.node_assignments.is_some());
        assert_eq!(annotated.selected_hierarchy, Some("location".to_string()));
    }

    #[test]
    fn test_json_serialization() {
        // Create a simple test case
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
            hierarchy_choices: Some(serde_json::json!([{"id": "location"}])),
            node_assignments: Some(serde_json::json!({"location": {"0": "loc_0"}})),
            selected_hierarchy: Some("location".to_string()),
            edge_style_config: Some(serde_json::json!({"default": "solid"})),
            node_type_config: Some(serde_json::json!({"defaultType": "Transform"})),
            legend: Some(serde_json::json!({"show": true})),
        };

        // Run analysis
        let results = run_analysis(&ir);

        // Serialize to JSON
        let json_result = annotate_and_serialize(&ir, &results);
        assert!(json_result.is_ok(), "JSON serialization should succeed");

        let json_str = json_result.unwrap();

        // Verify it's valid JSON by parsing it back
        let parsed: serde_json::Value = serde_json::from_str(&json_str)
            .expect("Serialized output should be valid JSON");

        // Verify key fields are present in the JSON
        assert!(parsed.get("nodes").is_some(), "JSON should have nodes field");
        assert!(parsed.get("edges").is_some(), "JSON should have edges field");
        assert!(parsed.get("overall").is_some(), "JSON should have overall field");

        // Verify nodes array has correct length
        let nodes_array = parsed["nodes"].as_array().expect("nodes should be an array");
        assert_eq!(nodes_array.len(), 3, "Should have 3 nodes");

        // Verify each node has analysis
        for node in nodes_array {
            assert!(node.get("analysis").is_some(), "Each node should have analysis");
            assert!(node.get("id").is_some(), "Each node should have id");
            assert!(node.get("nodeType").is_some(), "Each node should have nodeType");
        }

        // Verify edges array has correct length
        let edges_array = parsed["edges"].as_array().expect("edges should be an array");
        assert_eq!(edges_array.len(), 2, "Should have 2 edges");

        // Verify each edge has analysis
        for edge in edges_array {
            assert!(edge.get("analysis").is_some(), "Each edge should have analysis");
            assert!(edge.get("id").is_some(), "Each edge should have id");
            assert!(edge.get("source").is_some(), "Each edge should have source");
            assert!(edge.get("target").is_some(), "Each edge should have target");
        }

        // Verify overall analysis
        let overall = parsed["overall"].as_object().expect("overall should be an object");
        assert!(overall.get("deterministic").is_some(), "overall should have deterministic");
        assert!(overall.get("calm_safe").is_some(), "overall should have calm_safe");

        // Verify optional fields are preserved
        // Note: serde uses snake_case by default, but we need to check what's actually in the JSON
        // Let's check both camelCase and snake_case variants
        assert!(
            parsed.get("hierarchy_choices").is_some() || parsed.get("hierarchyChoices").is_some(),
            "JSON should have hierarchy_choices or hierarchyChoices"
        );
        assert!(
            parsed.get("node_assignments").is_some() || parsed.get("nodeAssignments").is_some(),
            "JSON should have node_assignments or nodeAssignments"
        );
        assert!(
            parsed.get("selected_hierarchy").is_some() || parsed.get("selectedHierarchy").is_some(),
            "JSON should have selected_hierarchy or selectedHierarchy"
        );
        assert!(
            parsed.get("edge_style_config").is_some() || parsed.get("edgeStyleConfig").is_some(),
            "JSON should have edge_style_config or edgeStyleConfig"
        );
        assert!(
            parsed.get("node_type_config").is_some() || parsed.get("nodeTypeConfig").is_some(),
            "JSON should have node_type_config or nodeTypeConfig"
        );
        assert!(parsed.get("legend").is_some(), "JSON should have legend");

        // Verify the JSON is pretty-printed (contains newlines)
        assert!(json_str.contains('\n'), "JSON should be pretty-printed with newlines");
    }

    #[test]
    fn test_json_round_trip() {
        // Create a test case
        let nodes = vec![
            make_test_node("0", "Source"),
            make_test_node("1", "NonDeterministic"),
            make_test_node("2", "Sink"),
        ];

        let edges = vec![
            make_test_edge("e0", "0", "1", vec!["Local"]),
            make_test_edge("e1", "1", "2", vec!["Network"]),
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

        // Run analysis
        let results = run_analysis(&ir);

        // Serialize to JSON
        let json_str = annotate_and_serialize(&ir, &results)
            .expect("Serialization should succeed");

        // Parse back to AnnotatedHydroIr
        let parsed: AnnotatedHydroIr = serde_json::from_str(&json_str)
            .expect("Should be able to parse back to AnnotatedHydroIr");

        // Verify structure is preserved
        assert_eq!(parsed.nodes.len(), 3);
        assert_eq!(parsed.edges.len(), 2);
        assert!(parsed.overall.is_some());

        // Verify analysis is present
        for node in &parsed.nodes {
            assert!(node.analysis.is_some());
        }

        for edge in &parsed.edges {
            assert!(edge.analysis.is_some());
        }

        // Serialize again and verify it's stable
        let json_str2 = serde_json::to_string_pretty(&parsed)
            .expect("Second serialization should succeed");

        assert_eq!(json_str, json_str2, "JSON serialization should be stable");
    }
}
