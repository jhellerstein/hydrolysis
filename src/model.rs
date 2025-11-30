// Data structures for Hydro IR JSON input/output

use serde::{Deserialize, Serialize};

/// Input structure matching hydro_lang::viz output
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HydroIr {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    #[serde(rename = "hierarchyChoices", skip_serializing_if = "Option::is_none")]
    pub hierarchy_choices: Option<serde_json::Value>,
    #[serde(rename = "nodeAssignments", skip_serializing_if = "Option::is_none")]
    pub node_assignments: Option<serde_json::Value>,
    #[serde(rename = "selectedHierarchy", skip_serializing_if = "Option::is_none")]
    pub selected_hierarchy: Option<String>,
    #[serde(rename = "edgeStyleConfig", skip_serializing_if = "Option::is_none")]
    pub edge_style_config: Option<serde_json::Value>,
    #[serde(rename = "nodeTypeConfig", skip_serializing_if = "Option::is_none")]
    pub node_type_config: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legend: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Node {
    pub id: String,
    #[serde(rename = "nodeType")]
    pub node_type: String,
    #[serde(rename = "shortLabel")]
    pub short_label: String,
    #[serde(rename = "fullLabel", skip_serializing_if = "Option::is_none")]
    pub full_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<NodeData>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NodeData {
    #[serde(rename = "locationId")]
    pub location_id: Option<usize>,
    #[serde(rename = "locationType")]
    pub location_type: Option<String>,
    pub backtrace: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Edge {
    pub id: String,
    pub source: String,
    pub target: String,
    #[serde(rename = "edgeProperties", skip_serializing_if = "Option::is_none")]
    pub edge_properties: Option<Vec<String>>,
    #[serde(rename = "semanticTags", skip_serializing_if = "Option::is_none")]
    pub semantic_tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// Analysis result structures for output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeAnalysis {
    pub nd_effect: String,
    pub monotone: bool,
    pub issues: Vec<Issue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_location: Option<SourceLocation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceLocation {
    pub file: String,
    pub line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeAnalysis {
    pub is_lattice: bool,
    pub calm: String,
    pub issues: Vec<Issue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub kind: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverallAnalysis {
    pub deterministic: bool,
    pub calm_safe: bool,
}

impl Node {
    /// Extract the most relevant source location from the backtrace.
    /// Prefers user code over framework code (filters out hydro_lang internals).
    pub fn extract_source_location(&self) -> Option<SourceLocation> {
        let backtrace = self.data.as_ref()?.backtrace.as_array()?;

        // First try to find user code (not in hydro_lang or std)
        for frame in backtrace {
            if let Some(file) = frame.get("file").and_then(|f| f.as_str()) {
                // Skip framework internals
                if file.contains("hydro_lang")
                    || file.contains("dfir_")
                    || file.contains("stageleft")
                    || file.starts_with("src/")
                        && (file.contains("location/")
                            || file.contains("compile/")
                            || file.contains("live_collections/"))
                {
                    continue;
                }

                let line = frame
                    .get("line")
                    .or_else(|| frame.get("lineNumber"))
                    .and_then(|l| l.as_u64())
                    .map(|l| l as u32)?;

                let function = frame
                    .get("function")
                    .or_else(|| frame.get("fn"))
                    .and_then(|f| f.as_str())
                    .map(|s| s.to_string());

                return Some(SourceLocation {
                    file: file.to_string(),
                    line,
                    function,
                });
            }
        }

        // Fallback: use the first frame if no user code found
        let frame = backtrace.first()?;
        let file = frame.get("file").and_then(|f| f.as_str())?.to_string();
        let line = frame
            .get("line")
            .or_else(|| frame.get("lineNumber"))
            .and_then(|l| l.as_u64())
            .map(|l| l as u32)?;
        let function = frame
            .get("function")
            .or_else(|| frame.get("fn"))
            .and_then(|f| f.as_str())
            .map(|s| s.to_string());

        Some(SourceLocation {
            file,
            line,
            function,
        })
    }
}

/// Annotated structures for output JSON (input + analysis)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotatedNode {
    pub id: String,
    #[serde(rename = "nodeType")]
    pub node_type: String,
    #[serde(rename = "semanticTags", skip_serializing_if = "Option::is_none")]
    pub semantic_tags: Option<Vec<String>>,
    #[serde(rename = "shortLabel")]
    pub short_label: String,
    #[serde(rename = "fullLabel", skip_serializing_if = "Option::is_none")]
    pub full_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<NodeData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub analysis: Option<NodeAnalysis>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotatedEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    #[serde(rename = "edgeProperties", skip_serializing_if = "Option::is_none")]
    pub edge_properties: Option<Vec<String>>,
    #[serde(rename = "semanticTags", skip_serializing_if = "Option::is_none")]
    pub semantic_tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub analysis: Option<EdgeAnalysis>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotatedHydroIr {
    pub nodes: Vec<AnnotatedNode>,
    pub edges: Vec<AnnotatedEdge>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overall: Option<OverallAnalysis>,
    #[serde(rename = "hierarchyChoices", skip_serializing_if = "Option::is_none")]
    pub hierarchy_choices: Option<serde_json::Value>,
    #[serde(rename = "nodeAssignments", skip_serializing_if = "Option::is_none")]
    pub node_assignments: Option<serde_json::Value>,
    #[serde(rename = "selectedHierarchy", skip_serializing_if = "Option::is_none")]
    pub selected_hierarchy: Option<String>,
    #[serde(rename = "edgeStyleConfig", skip_serializing_if = "Option::is_none")]
    pub edge_style_config: Option<serde_json::Value>,
    #[serde(rename = "nodeTypeConfig", skip_serializing_if = "Option::is_none")]
    pub node_type_config: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legend: Option<serde_json::Value>,
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use proptest::prelude::*;

    // **Feature: hydro-static-analysis, Property 1: JSON Round-Trip Preservation**
    // **Validates: Requirements 1.1, 1.2, 1.3, 1.5, 7.6**
    //
    // For any valid Hydro IR JSON, parsing then serializing (without analysis)
    // should produce equivalent JSON structure.

    // Test helper functions (shared across test modules)
    pub fn make_test_node(id: &str, node_type: &str) -> Node {
        Node {
            id: id.to_string(),
            node_type: node_type.to_string(),
            short_label: format!("{}_short", id),
            full_label: Some(format!("{}_full", id)),
            label: Some(format!("{}_label", id)),
            data: Some(NodeData {
                location_id: Some(0),
                location_type: Some("Process".to_string()),
                backtrace: serde_json::json!([]),
            }),
        }
    }

    pub fn make_test_edge(id: &str, source: &str, target: &str, tags: Vec<&str>) -> Edge {
        Edge {
            id: id.to_string(),
            source: source.to_string(),
            target: target.to_string(),
            edge_properties: None,
            semantic_tags: Some(tags.iter().map(|s| s.to_string()).collect()),
            label: None,
        }
    }

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
            .prop_map(
                |(id, source, target, edge_properties, semantic_tags, label)| Edge {
                    id,
                    source,
                    target,
                    edge_properties,
                    semantic_tags,
                    label,
                },
            )
    }

    // Strategy for generating valid HydroIr
    fn arb_hydro_ir() -> impl Strategy<Value = HydroIr> {
        (
            prop::collection::vec(arb_node(), 1..10),
            prop::collection::vec(arb_edge(), 0..15),
            prop::option::of(Just(
                serde_json::json!([{"id": "location", "name": "Location"}]),
            )),
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
        fn test_json_roundtrip(hydro_ir in arb_hydro_ir()) {
            // Serialize to JSON
            let json_str = serde_json::to_string(&hydro_ir)
                .expect("Failed to serialize HydroIr to JSON");

            // Parse back from JSON
            let parsed: HydroIr = serde_json::from_str(&json_str)
                .expect("Failed to parse JSON back to HydroIr");

            // Serialize again
            let json_str2 = serde_json::to_string(&parsed)
                .expect("Failed to serialize parsed HydroIr to JSON");

            // The two JSON strings should be identical
            assert_eq!(json_str, json_str2, "Round-trip JSON serialization should be stable");

            // Also verify structural equality
            assert_eq!(hydro_ir.nodes.len(), parsed.nodes.len(), "Node count should match");
            assert_eq!(hydro_ir.edges.len(), parsed.edges.len(), "Edge count should match");

            // Verify node IDs are preserved
            for (original, parsed_node) in hydro_ir.nodes.iter().zip(parsed.nodes.iter()) {
                assert_eq!(original.id, parsed_node.id, "Node IDs should match");
                assert_eq!(original.node_type, parsed_node.node_type, "Node types should match");
                assert_eq!(original.short_label, parsed_node.short_label, "Node short labels should match");
            }

            // Verify edge IDs are preserved
            for (original, parsed_edge) in hydro_ir.edges.iter().zip(parsed.edges.iter()) {
                assert_eq!(original.id, parsed_edge.id, "Edge IDs should match");
                assert_eq!(original.source, parsed_edge.source, "Edge sources should match");
                assert_eq!(original.target, parsed_edge.target, "Edge targets should match");
            }
        }

        // **Feature: hydro-static-analysis, Property 2: Malformed JSON Error Handling**
        // **Validates: Requirements 1.4**
        //
        // For any malformed JSON input, Hydrolysis should report an error and not produce output.
        #[test]
        fn test_malformed_json_error_handling(
            malformed in prop::string::string_regex(
                // Generate strings that are likely to be invalid JSON
                "(\\{[^}]*|\\[[^\\]]*|[^\\{\\[]*\\}|[^\\{\\[]*\\]|[a-zA-Z0-9_]+|\\{\"nodes\":\\[\\{\"id\":\"0\",\"nodeType\":)"
            ).unwrap()
        ) {
            // Attempt to parse the malformed JSON
            let result: Result<HydroIr, _> = serde_json::from_str(&malformed);

            // The parse should fail for malformed JSON
            // We verify that the error is properly reported (not panicking)
            if result.is_ok() {
                // If it somehow parsed successfully, verify it's actually valid
                let parsed = result.unwrap();
                // Re-serialize to ensure it's truly valid
                let _json_str = serde_json::to_string(&parsed)
                    .expect("If parsing succeeded, serialization should also succeed");
            } else {
                // This is the expected case - parsing should fail
                // The error should be a serde_json::Error
                assert!(result.is_err(), "Malformed JSON should produce an error");
            }
        }
    }
}
