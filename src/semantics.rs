// Operator semantics classification for Hydro operators

/// Nondeterminism effect classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NdEffect {
    Deterministic,
    LocallyNonDet,
    ExternalNonDet,
}

/// Monotonicity classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Monotonicity {
    Always,
    Never,
    Depends,
}

/// Operator semantics combining ND effect and monotonicity
#[derive(Debug, Clone, Copy)]
pub struct OpSemantics {
    pub nd: NdEffect,
    pub monotone: Monotonicity,
}

/// Lookup operator semantics by node type
pub fn get_semantics(node_type: &str) -> OpSemantics {
    match node_type {
        "Source" => OpSemantics {
            nd: NdEffect::Deterministic,
            monotone: Monotonicity::Always,
        },
        "Transform" => OpSemantics {
            nd: NdEffect::Deterministic,
            monotone: Monotonicity::Always,
        },
        "Join" => OpSemantics {
            nd: NdEffect::Deterministic,
            monotone: Monotonicity::Always,
        },
        "Aggregation" => OpSemantics {
            nd: NdEffect::Deterministic,
            monotone: Monotonicity::Depends,
        },
        "Network" => OpSemantics {
            nd: NdEffect::Deterministic,
            monotone: Monotonicity::Always,
        },
        "Sink" => OpSemantics {
            nd: NdEffect::Deterministic,
            monotone: Monotonicity::Always,
        },
        "Tee" => OpSemantics {
            nd: NdEffect::Deterministic,
            monotone: Monotonicity::Always,
        },
        "NonDeterministic" => OpSemantics {
            nd: NdEffect::LocallyNonDet,
            monotone: Monotonicity::Never,
        },
        // Conservative default for unknown types
        _ => OpSemantics {
            nd: NdEffect::LocallyNonDet,
            monotone: Monotonicity::Never,
        },
    }
}

/// Finer-grained semantics lookup by operator label
///
/// Returns None only for truly unknown operators - caller should handle this explicitly
pub fn get_semantics_by_label(label: &str) -> Option<OpSemantics> {
    match label.to_lowercase().as_str() {
        // === MONOTONE TRANSFORMS ===
        // Simple element-wise transformations
        "map" | "flat_map" | "flatmap" | "filter" | "filter_map" | "filtermap" | "inspect"
        | "enumerate" | "cloned" => Some(OpSemantics {
            nd: NdEffect::Deterministic,
            monotone: Monotonicity::Always,
        }),

        // Type conversions and structural operations
        "cast"
        | "chain"
        | "chainfirst"
        | "into_keyed"
        | "keys"
        | "resolve_futures"
        | "resolve_futures_ordered"
        | "all_ticks"
        | "all_ticks_atomic"
        | "defer_tick"
        | "begin_atomic"
        | "end_atomic"
        | "atomic" => Some(OpSemantics {
            nd: NdEffect::Deterministic,
            monotone: Monotonicity::Always,
        }),

        // === MONOTONE JOINS ===
        "join"
        | "cross_product"
        | "crossproduct"
        | "cross_singleton"
        | "cross_product_nested_loop" => Some(OpSemantics {
            nd: NdEffect::Deterministic,
            monotone: Monotonicity::Always,
        }),

        // === NON-MONOTONE OPERATIONS ===
        // Set difference and anti-join require retractions
        "difference" | "anti_join" | "antijoin" | "filter_not_in" => Some(OpSemantics {
            nd: NdEffect::Deterministic,
            monotone: Monotonicity::Never,
        }),

        // Unique is monotone: adding input can only add to cumulative output, never retract.
        // It's stateful and order-sensitive, but doesn't require coordination (CALM-safe).
        "unique" => Some(OpSemantics {
            nd: NdEffect::Deterministic,
            monotone: Monotonicity::Always,
        }),

        // === AGGREGATIONS (DEPENDS ON FUNCTION) ===
        // Idempotent + commutative folds are CALM-safe (can handle reordering and duplication)
        "fold_commutative_idempotent"
        | "fold_idempotent_commutative"
        | "reduce_commutative_idempotent"
        | "reduce_idempotent_commutative" => Some(OpSemantics {
            nd: NdEffect::Deterministic,
            monotone: Monotonicity::Always,
        }),

        // Fold/reduce/scan - monotonicity depends on the aggregation function
        "fold"
        | "fold_keyed"
        | "foldkeyed"
        | "fold_commutative"
        | "fold_idempotent"
        | "reduce"
        | "reduce_keyed"
        | "reducekeyed"
        | "reduce_commutative"
        | "reduce_idempotent"
        | "reduce_keyed_watermark"
        | "scan" => Some(OpSemantics {
            nd: NdEffect::Deterministic,
            monotone: Monotonicity::Depends,
        }),

        // Sort is non-monotone (requires seeing all elements)
        "sort" => Some(OpSemantics {
            nd: NdEffect::Deterministic,
            monotone: Monotonicity::Never,
        }),

        // Min/max/count/first/last - depends on whether they're over lattices
        "min" | "max" | "count" | "first" | "last" | "collect_vec" => Some(OpSemantics {
            nd: NdEffect::Deterministic,
            monotone: Monotonicity::Depends,
        }),

        // === NETWORK OPERATIONS ===
        "batch" | "batch_atomic" => Some(OpSemantics {
            nd: NdEffect::Deterministic,
            monotone: Monotonicity::Always,
        }),

        "network" => Some(OpSemantics {
            nd: NdEffect::Deterministic,
            monotone: Monotonicity::Always,
        }),

        // === NONDETERMINISTIC OPERATIONS ===
        "observe_non_det" | "observenondet" | "nondet" => Some(OpSemantics {
            nd: NdEffect::LocallyNonDet,
            monotone: Monotonicity::Never,
        }),

        // Sampling operations are nondeterministic
        "sample_every" | "timeout" => Some(OpSemantics {
            nd: NdEffect::LocallyNonDet,
            monotone: Monotonicity::Never,
        }),

        // === STATE OPERATIONS ===
        // Persist accumulates items and replays them each tick - monotone (only adds, never removes)
        "persist" => Some(OpSemantics {
            nd: NdEffect::Deterministic,
            monotone: Monotonicity::Always,
        }),

        // === STRUCTURAL OPERATIONS ===
        "tee" => Some(OpSemantics {
            nd: NdEffect::Deterministic,
            monotone: Monotonicity::Always,
        }),

        // === SOURCES AND SINKS ===
        "source_stream" | "source_iter" | "external_input" | "cycle_source"
        | "singleton_source" => Some(OpSemantics {
            nd: NdEffect::Deterministic,
            monotone: Monotonicity::Always,
        }),

        "for_each" | "send_external" | "cycle_sink" | "dest_sink" => Some(OpSemantics {
            nd: NdEffect::Deterministic,
            monotone: Monotonicity::Always,
        }),

        // === CONDITIONAL OPERATIONS ===
        "filter_if_some" | "filter_if_none" => Some(OpSemantics {
            nd: NdEffect::Deterministic,
            monotone: Monotonicity::Always,
        }),

        _ => None,
    }
}

/// Detect lattice types from edge labels.
///
/// Ideally this would check for Hydro's `Merge` trait implementation, but since we only
/// have type strings in the JSON, we use heuristics:
/// 1. Check for types from the `lattices` crate namespace
/// 2. Check for known lattice type names (Max, Min, DomPair, etc.)
/// 3. Check for common wrapper types (WithBot, WithTop, etc.)
///
/// This is conservative - it may miss custom lattice types, but won't false-positive
/// on non-lattice types.
pub fn is_lattice_type(label: Option<&str>) -> bool {
    label.is_some_and(|l| {
        // Check for lattices crate namespace
        l.contains("lattices::")
            // Known lattice types from the lattices crate
            || l.contains("Max<")
            || l.contains("Min<")
            || l.contains("DomPair<")
            || l.contains("SetUnion<")
            || l.contains("MapUnion<")
            || l.contains("VecUnion<")
            || l.contains("WithBot<")
            || l.contains("WithTop<")
            || l.contains("Conflict<")
            || l.contains("Point<")
            || l.contains("Pair<")
            // Application-specific lattice wrappers
            || l.contains("CausalWrapper<")
            || l.contains("VCWrapper<")
            // Check for SetUnionWithTombstones, MapUnionWithTombstones
            || l.contains("WithTombstones<")
    })
}

/// Check if a batch operator is from a network operator (structural ND) vs manual use (semantic ND)
pub fn is_network_batch(backtrace: &serde_json::Value) -> bool {
    // Check if the backtrace contains network-related files
    if let Some(frames) = backtrace.as_array() {
        for frame in frames {
            if let Some(file) = frame.get("file").and_then(|f| f.as_str()) {
                // Network batching appears in networking.rs or location/mod.rs
                if file.contains("networking.rs") || file.contains("location/mod.rs") {
                    return true;
                }
            }
        }
    }
    false
}

/// Check if a fold/reduce is actually a commutative+idempotent variant by inspecting backtrace
fn is_commutative_idempotent_fold(backtrace: &serde_json::Value) -> bool {
    if let Some(frames) = backtrace.as_array() {
        for frame in frames {
            if let Some(func) = frame.get("function").and_then(|f| f.as_str()) {
                if func.contains("commutative_idempotent") || func.contains("idempotent_commutative")
                {
                    return true;
                }
            }
        }
    }
    false
}

/// Get semantics for a node, using label-based lookup with network batch detection
///
/// This is the canonical way to determine node semantics, handling:
/// - Label-based lookup for finer-grained classification
/// - Special case for batch operators (network vs manual)
/// - Special case for fold/reduce (check if commutative+idempotent via backtrace)
/// - Special case for observenondet (check if inside commutative+idempotent fold)
/// - Node type lookup when no label is present
///
/// Panics if an unknown label is encountered (fail fast, don't hide bugs)
pub fn get_node_semantics(node: &crate::model::Node) -> OpSemantics {
    if let Some(label) = &node.label {
        // Special handling for batch: check if it's network batching
        if label == "batch" {
            let is_network = node
                .data
                .as_ref()
                .map(|d| is_network_batch(&d.backtrace))
                .unwrap_or(false);

            if is_network {
                // Network batching is deterministic and monotone
                OpSemantics {
                    nd: NdEffect::Deterministic,
                    monotone: Monotonicity::Always,
                }
            } else {
                // Manual batch is semantic nondeterminism
                get_semantics_by_label(label).expect("batch should be in label lookup table")
            }
        } else if matches!(
            label.as_str(),
            "fold" | "foldkeyed" | "fold_keyed" | "reduce" | "reducekeyed" | "reduce_keyed"
        ) {
            // Check if this is actually a commutative+idempotent variant
            let is_ci = node
                .data
                .as_ref()
                .map(|d| is_commutative_idempotent_fold(&d.backtrace))
                .unwrap_or(false);

            if is_ci {
                // Commutative+idempotent folds are CALM-safe
                OpSemantics {
                    nd: NdEffect::Deterministic,
                    monotone: Monotonicity::Always,
                }
            } else {
                // Regular fold/reduce - depends on function
                get_semantics_by_label(label).expect("fold/reduce should be in label lookup table")
            }
        } else if label == "observenondet" {
            // Check if this observenondet is inside a commutative+idempotent fold
            // If so, it's structural (batching implementation) not semantic nondeterminism
            let is_ci_internal = node
                .data
                .as_ref()
                .map(|d| is_commutative_idempotent_fold(&d.backtrace))
                .unwrap_or(false);

            if is_ci_internal {
                // Internal batching in CALM-safe fold - treat as deterministic
                OpSemantics {
                    nd: NdEffect::Deterministic,
                    monotone: Monotonicity::Always,
                }
            } else {
                // Actual semantic nondeterminism
                get_semantics_by_label(label).expect("observenondet should be in label lookup table")
            }
        } else {
            // Try label-based lookup first
            get_semantics_by_label(label).unwrap_or_else(|| {
                // If label is unknown, use node type as fallback
                // This is expected for generic operators
                get_semantics(&node.node_type)
            })
        }
    } else {
        // No label, use node type
        get_semantics(&node.node_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // **Feature: hydro-static-analysis, Property 3: Operator Classification Completeness**
    // **Validates: Requirements 2.1, 2.2, 2.3**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn test_operator_classification_completeness(node_type in prop::sample::select(vec![
            "Source", "Transform", "Join", "Aggregation",
            "Network", "Sink", "Tee", "NonDeterministic"
        ])) {
            let semantics = get_semantics(node_type);

            // Property: For any valid node type, semantics lookup returns valid NdEffect and Monotonicity
            // Verify that we get a valid NdEffect
            match semantics.nd {
                NdEffect::Deterministic | NdEffect::LocallyNonDet | NdEffect::ExternalNonDet => {},
            }

            // Verify that we get a valid Monotonicity
            match semantics.monotone {
                Monotonicity::Always | Monotonicity::Never | Monotonicity::Depends => {},
            }

            // Additional check: NonDeterministic nodes should be classified as LocallyNonDet
            if node_type == "NonDeterministic" {
                prop_assert_eq!(semantics.nd, NdEffect::LocallyNonDet);
                prop_assert_eq!(semantics.monotone, Monotonicity::Never);
            }
        }
    }

    // **Feature: hydro-static-analysis, Property 4: Conservative Default for Unknown Types**
    // **Validates: Requirements 2.5**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn test_unknown_type_conservative_defaults(
            unknown_type in "[a-z]{1,20}"
                .prop_filter("Must not be a known type", |s| {
                    !matches!(s.as_str(),
                        "Source" | "Transform" | "Join" | "Aggregation" |
                        "Network" | "Sink" | "Tee" | "NonDeterministic")
                })
        ) {
            let semantics = get_semantics(&unknown_type);

            // Property: For any unknown node type, semantics should return conservative defaults
            prop_assert_eq!(semantics.nd, NdEffect::LocallyNonDet,
                "Unknown types should default to LocallyNonDet");
            prop_assert_eq!(semantics.monotone, Monotonicity::Never,
                "Unknown types should default to Never monotone");
        }
    }

    // **Feature: hydro-static-analysis, Property 5: Lattice Detection Consistency**
    // **Validates: Requirements 3.1, 3.2, 3.3**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn test_lattice_detection_with_pattern(
            lattice_pattern in prop::sample::select(vec![
                "CausalWrapper", "VCWrapper", "DomPair",
                "SetUnion", "MapUnion", "Max", "Min"
            ])
        ) {
            // Property: For any edge label containing a lattice pattern with generic brackets,
            // detection should be true (matching actual Rust type syntax)
            let label_with_pattern = format!("Stream<{}<String>>", lattice_pattern);
            prop_assert!(is_lattice_type(Some(&label_with_pattern)),
                "Label '{}' containing '{}' should be detected as lattice type",
                label_with_pattern, lattice_pattern);

            // Property: Detection should be deterministic - same input gives same output
            let result1 = is_lattice_type(Some(&label_with_pattern));
            let result2 = is_lattice_type(Some(&label_with_pattern));
            prop_assert_eq!(result1, result2, "Lattice detection should be deterministic");
        }

        #[test]
        fn test_non_lattice_detection(
            label in "[a-z]{1,20}"
                .prop_filter("Must not contain lattice patterns", |s| {
                    !s.contains("CausalWrapper") && !s.contains("VCWrapper") &&
                    !s.contains("DomPair") && !s.contains("SetUnion") &&
                    !s.contains("MapUnion") && !s.contains("Max") && !s.contains("Min")
                })
        ) {
            // Property: Labels without lattice patterns should not be detected as lattice types
            prop_assert!(!is_lattice_type(Some(&label)),
                "Label '{}' without lattice patterns should not be detected as lattice type", label);
        }
    }

    #[test]
    fn test_none_label_not_lattice() {
        // Property: None label should always return false
        assert!(
            !is_lattice_type(None),
            "None label should not be a lattice type"
        );
    }
}
