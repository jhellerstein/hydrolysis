// Report generation for analysis results

use crate::analysis::AnalysisResult;
use crate::model::HydroIr;

// Report formatting constants
const MAX_REPORT_OPERATIONS: usize = 20;
const MAX_REPORT_EDGES: usize = 15;

/// Generate a human-readable report of analysis findings
pub fn generate_report(ir: &HydroIr, results: &AnalysisResult) -> String {
    let mut report = String::new();
    
    report.push_str("=== Hydrolysis Analysis Report ===\n\n");
    
    // Overall summary
    report.push_str("OVERALL SUMMARY:\n");
    report.push_str(&format!("  Deterministic: {}\n", 
        if results.overall.deterministic { "✓ YES" } else { "✗ NO" }));
    report.push_str(&format!("  CALM Safe: {}\n\n", 
        if results.overall.calm_safe { "✓ YES" } else { "✗ NO" }));
    
    // Count root causes (not inherited issues)
    use crate::semantics::{get_node_semantics, NdEffect, Monotonicity};
    
    let mut nondet_root_count = 0;
    let mut nonmonotone_root_count = 0;
    let mut nonlattice_root_count = 0;
    
    // Count root cause nodes (intrinsically non-deterministic or non-monotone)
    for node in &ir.nodes {
        let semantics = get_node_semantics(node);
        
        if semantics.nd != NdEffect::Deterministic {
            nondet_root_count += 1;
        }
        
        if semantics.monotone == Monotonicity::Never {
            nonmonotone_root_count += 1;
        }
    }
    
    // Count non-lattice edges on CALM-critical paths
    for edge in &ir.edges {
        if let Some(analysis) = results.edge_analyses.get(&edge.id) {
            if !analysis.is_lattice && analysis.issues.iter().any(|i| i.kind == "NonLattice") {
                nonlattice_root_count += 1;
            }
        }
    }
    
    report.push_str("ROOT CAUSE SUMMARY:\n");
    report.push_str(&format!("  Nondeterministic operations: {}\n", nondet_root_count));
    report.push_str(&format!("  Non-monotone operations: {}\n", nonmonotone_root_count));
    report.push_str(&format!("  Non-lattice edges: {}\n\n", nonlattice_root_count));
    
    // CALM violations
    if !results.overall.calm_safe {
        report.push_str("CALM ANALYSIS:\n\n");
        
        // Collect root causes
        let mut nondet_nodes = Vec::new();
        let mut nonmonotone_nodes = Vec::new();
        let mut root_cause_edges = Vec::new();
        
        // Find root cause nodes (intrinsically non-deterministic or non-monotone)
        for node in &ir.nodes {
            let semantics = get_node_semantics(node);
            
            // Check if node is intrinsically non-deterministic (not just tainted)
            if semantics.nd != NdEffect::Deterministic {
                nondet_nodes.push(node);
            }
            
            // Check if node is intrinsically non-monotone
            if semantics.monotone == Monotonicity::Never {
                nonmonotone_nodes.push(node);
            }
        }
        
        // Find non-lattice edges that are themselves the problem
        for edge in &ir.edges {
            if let Some(analysis) = results.edge_analyses.get(&edge.id) {
                if !analysis.is_lattice && analysis.issues.iter().any(|i| i.kind == "NonLattice") {
                    root_cause_edges.push(edge);
                }
            }
        }
        
        // Report root causes in table format
        report.push_str("ROOT CAUSES:\n\n");
        
        // Build combined table of root cause operations
        use std::collections::HashSet;
        let nondet_ids: HashSet<_> = nondet_nodes.iter().map(|n| &n.id).collect();
        let nonmono_ids: HashSet<_> = nonmonotone_nodes.iter().map(|n| &n.id).collect();
        
        let mut op_rows = Vec::new();
        for node in &ir.nodes {
            let is_nondet = nondet_ids.contains(&node.id);
            let is_nonmono = nonmono_ids.contains(&node.id);
            
            if is_nondet || is_nonmono {
                let source_loc = results.node_analyses.get(&node.id)
                    .and_then(|a| a.source_location.as_ref())
                    .map(|loc| format!("{}:{}", loc.file, loc.line))
                    .unwrap_or_else(|| "?".to_string());
                
                op_rows.push((
                    node.short_label.clone(),
                    node.id.clone(),
                    node.node_type.clone(),
                    is_nondet,
                    is_nonmono,
                    source_loc,
                ));
            }
        }
        
        if !op_rows.is_empty() {
            report.push_str("  Operations:\n");
            report.push_str("  ┌────────────────────┬──────┬──────────────────┬─────────┬──────────┬──────────────────────────────┐\n");
            report.push_str("  │ Op Label           │ ID   │ Type             │ NonDet  │ NonMono  │ Source Location              │\n");
            report.push_str("  ├────────────────────┼──────┼──────────────────┼─────────┼──────────┼──────────────────────────────┤\n");
            
            for (label, id, node_type, is_nondet, is_nonmono, source_loc) in op_rows.iter().take(MAX_REPORT_OPERATIONS) {
                let label_trunc = if label.len() > 18 {
                    format!("{}…", &label[..17])
                } else {
                    format!("{:<18}", label)
                };
                let id_trunc = if id.len() > 4 {
                    format!("{}…", &id[..3])
                } else {
                    format!("{:<4}", id)
                };
                let type_trunc = if node_type.len() > 16 {
                    format!("{}…", &node_type[..15])
                } else {
                    format!("{:<16}", node_type)
                };
                let loc_trunc = if source_loc.len() > 28 {
                    format!("…{}", &source_loc[source_loc.len()-27..])
                } else {
                    format!("{:<28}", source_loc)
                };
                let nondet_icon = if *is_nondet { "✗" } else { "✓" };
                let nonmono_icon = if *is_nonmono { "✗" } else { "✓" };
                
                report.push_str(&format!("  │ {} │ {} │ {} │   {}     │    {}     │ {} │\n",
                    label_trunc, id_trunc, type_trunc, nondet_icon, nonmono_icon, loc_trunc));
            }
            
            if op_rows.len() > MAX_REPORT_OPERATIONS {
                report.push_str(&format!("  │ ... and {} more operations\n", op_rows.len() - MAX_REPORT_OPERATIONS));
            }
            
            report.push_str("  └────────────────────┴──────┴──────────────────┴─────────┴──────────┴──────────────────────────────┘\n");
            report.push_str("  Legend: ✓ = deterministic/monotone, ✗ = non-deterministic/non-monotone\n\n");
        }
        
        if !root_cause_edges.is_empty() {
            report.push_str("  Non-lattice Edges:\n");
            report.push_str("  ┌──────┬────────────────────┬────────────────────┬──────────┬──────────────────────────────┐\n");
            report.push_str("  │ ID   │ Source             │ Target             │ Lattice  │ Source Location              │\n");
            report.push_str("  ├──────┼────────────────────┼────────────────────┼──────────┼──────────────────────────────┤\n");
            
            for edge in root_cause_edges.iter().take(MAX_REPORT_EDGES) {
                let source_node = ir.nodes.iter()
                    .find(|n| n.id == edge.source)
                    .map(|n| n.short_label.as_str())
                    .unwrap_or("?");
                let target_node = ir.nodes.iter()
                    .find(|n| n.id == edge.target)
                    .map(|n| n.short_label.as_str())
                    .unwrap_or("?");
                
                // Get source location from the source node
                let source_loc = results.node_analyses.get(&edge.source)
                    .and_then(|a| a.source_location.as_ref())
                    .map(|loc| format!("{}:{}", loc.file, loc.line))
                    .unwrap_or_else(|| "?".to_string());
                
                let id_trunc = if edge.id.len() > 4 {
                    format!("{}…", &edge.id[..3])
                } else {
                    format!("{:<4}", edge.id)
                };
                let src_trunc = if source_node.len() > 18 {
                    format!("{}…", &source_node[..17])
                } else {
                    format!("{:<18}", source_node)
                };
                let tgt_trunc = if target_node.len() > 18 {
                    format!("{}…", &target_node[..17])
                } else {
                    format!("{:<18}", target_node)
                };
                let loc_trunc = if source_loc.len() > 28 {
                    format!("…{}", &source_loc[source_loc.len()-27..])
                } else {
                    format!("{:<28}", source_loc)
                };
                
                report.push_str(&format!("  │ {} │ {} │ {} │    ✗     │ {} │\n",
                    id_trunc, src_trunc, tgt_trunc, loc_trunc));
            }
            
            if root_cause_edges.len() > MAX_REPORT_EDGES {
                report.push_str(&format!("  │ ... and {} more edges\n", root_cause_edges.len() - MAX_REPORT_EDGES));
            }
            
            report.push_str("  └──────┴────────────────────┴────────────────────┴──────────┴──────────────────────────────┘\n");
            report.push_str("  Legend: ✓ = lattice type, ✗ = non-lattice type\n\n");
        }
        
        // Report downstream effects on CALM-critical edges
        report.push_str("DOWNSTREAM EFFECTS:\n");
        report.push_str("  CALM-critical edges affected by root causes:\n\n");
        
        let mut critical_edges = Vec::new();
        for edge in &ir.edges {
            let is_network = edge.semantic_tags.as_ref()
                .map(|tags| tags.iter().any(|tag| tag == "Network"))
                .unwrap_or(false);
            
            let targets_sink = ir.nodes.iter()
                .find(|n| n.id == edge.target)
                .map(|n| n.node_type == "Sink")
                .unwrap_or(false);
            
            if is_network || targets_sink {
                if let Some(analysis) = results.edge_analyses.get(&edge.id) {
                    if analysis.calm == "CalmUnsafe" {  // Note: using string literal to match EdgeAnalysis.calm field
                        let source_node = ir.nodes.iter()
                            .find(|n| n.id == edge.source)
                            .map(|n| n.short_label.as_str())
                            .unwrap_or("?");
                        let target_node = ir.nodes.iter()
                            .find(|n| n.id == edge.target)
                            .map(|n| n.short_label.as_str())
                            .unwrap_or("?");
                        
                        let edge_type = if is_network { "Net" } else { "Sink" };
                        let is_root = !analysis.is_lattice;
                        
                        // Get source location from the source node
                        let source_loc = results.node_analyses.get(&edge.source)
                            .and_then(|a| a.source_location.as_ref())
                            .map(|loc| format!("{}:{}", loc.file, loc.line))
                            .unwrap_or_else(|| "?".to_string());
                        
                        critical_edges.push((
                            edge.id.clone(),
                            source_node.to_string(),
                            target_node.to_string(),
                            edge_type,
                            is_root,
                            source_loc,
                        ));
                    }
                }
            }
        }
        
        if !critical_edges.is_empty() {
            report.push_str("  ┌──────┬────────────────────┬────────────────────┬──────┬──────────┬──────────────────────────────┐\n");
            report.push_str("  │ ID   │ Source             │ Target             │ Type │ Cause    │ Source Location              │\n");
            report.push_str("  ├──────┼────────────────────┼────────────────────┼──────┼──────────┼──────────────────────────────┤\n");
            
            for (id, source, target, edge_type, is_root, source_loc) in critical_edges {
                let id_trunc = if id.len() > 4 {
                    format!("{}…", &id[..3])
                } else {
                    format!("{:<4}", id)
                };
                let src_trunc = if source.len() > 18 {
                    format!("{}…", &source[..17])
                } else {
                    format!("{:<18}", source)
                };
                let tgt_trunc = if target.len() > 18 {
                    format!("{}…", &target[..17])
                } else {
                    format!("{:<18}", target)
                };
                let loc_trunc = if source_loc.len() > 28 {
                    format!("…{}", &source_loc[source_loc.len()-27..])
                } else {
                    format!("{:<28}", source_loc)
                };
                let cause = if is_root { "Root" } else { "Inherit" };
                
                report.push_str(&format!("  │ {} │ {} │ {} │ {:<4} │ {:<8} │ {} │\n",
                    id_trunc, src_trunc, tgt_trunc, edge_type, cause, loc_trunc));
            }
            
            report.push_str("  └──────┴────────────────────┴────────────────────┴──────┴──────────┴──────────────────────────────┘\n");
            report.push_str("  Legend: Root = edge itself non-lattice, Inherit = upstream causes\n\n");
        }
    }
    
    if results.overall.deterministic && results.overall.calm_safe {
        report.push_str("\n✓ No issues found! Your dataflow is deterministic and CALM-safe.\n\n");
    }
    
    report.push_str("=== End of Report ===\n");
    
    report
}
