// CLI entry point for Hydrolysis static analysis tool

use anyhow::{Context, Result};
use std::env;
use std::fs;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() != 3 {
        eprintln!("Usage: {} <input.json> <output.json>", args[0]);
        std::process::exit(1);
    }

    let input_path = &args[1];
    let output_path = &args[2];

    // Read input JSON
    let input_json = fs::read_to_string(input_path)
        .with_context(|| format!("Failed to read input file: {}", input_path))?;

    // Parse Hydro IR
    let ir: hydrolysis::model::HydroIr = serde_json::from_str(&input_json)
        .with_context(|| "Failed to parse input JSON")?;

    // Run analysis
    let results = hydrolysis::analysis::run_analysis(&ir);

    // Generate and print report
    let report = hydrolysis::report::generate_report(&ir, &results);
    println!("{}", report);

    // Annotate and serialize output
    let output_json = hydrolysis::annotate::annotate_and_serialize(&ir, &results)?;

    // Write output
    fs::write(output_path, output_json)
        .with_context(|| format!("Failed to write output file: {}", output_path))?;

    Ok(())
}
