use anyhow::{Context, Result};
use std::path::PathBuf;

/// Generate CI configs from cigen.yml
pub fn generate_command(file: Option<String>, output: Option<String>) -> Result<()> {
    // Find cigen.yml
    let config_path = find_cigen_yml(file)?;

    println!("Loading config from: {}", config_path.display());

    // Load and parse config (handle both single file and directory)
    let config = if config_path.is_dir() {
        cigen::loader::load_split_config(&config_path)?
    } else {
        let yaml = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;
        cigen::schema::CigenConfig::from_yaml(&yaml).context("Failed to parse cigen.yml")?
    };

    println!("Parsed config with {} job(s)", config.jobs.len());

    // Determine plugin directory (where provider binaries are)
    let plugin_dir = determine_plugin_dir();
    println!("Using plugin directory: {}", plugin_dir.display());

    // Create orchestrator
    let mut orchestrator = cigen::orchestrator::WorkflowOrchestrator::new(plugin_dir);

    // Execute workflow
    println!("Executing workflow...");
    let runtime = tokio::runtime::Runtime::new()?;
    let result = runtime.block_on(orchestrator.execute(config))?;

    // Write output files
    let output_dir = output
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    println!("\nGenerated {} file(s):", result.files.len());
    for (path, content) in &result.files {
        let full_path = output_dir.join(path);

        // Create parent directories if needed
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&full_path, content)
            .with_context(|| format!("Failed to write file: {}", full_path.display()))?;

        println!("  ✓ {}", path);
    }

    println!("\n✨ Done!");

    Ok(())
}

/// Find cigen.yml in various locations
fn find_cigen_yml(file: Option<String>) -> Result<PathBuf> {
    if let Some(path) = file {
        let p = PathBuf::from(path);
        if p.exists() {
            return Ok(p);
        } else {
            anyhow::bail!("Config file not found: {}", p.display());
        }
    }

    // Try common locations
    let candidates = vec![
        PathBuf::from("cigen.yml"),
        PathBuf::from(".cigen"), // directory with split config
        PathBuf::from(".cigen/cigen.yml"),
        PathBuf::from("cigen.yaml"),
        PathBuf::from(".cigen/cigen.yaml"),
    ];

    for candidate in candidates {
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    anyhow::bail!(
        "No cigen config found. Tried: cigen.yml, .cigen/, .cigen/cigen.yml, cigen.yaml, .cigen/cigen.yaml"
    )
}

/// Determine where plugin binaries are located
fn determine_plugin_dir() -> PathBuf {
    // In development, use target/debug
    // In production, use the same directory as the cigen binary

    // Check if we're in development (target/debug or target/release exists)
    let cargo_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let debug_dir = cargo_dir.join("target/debug");
    let release_dir = cargo_dir.join("target/release");

    if debug_dir.exists() {
        return debug_dir;
    } else if release_dir.exists() {
        return release_dir;
    }

    // Production: same directory as binary
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}
