//! Command-specific loading logic

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

use super::file_scanner::FileScanner;
use crate::models::Command;
use crate::templating::TemplateEngine;

pub struct CommandLoader<'a> {
    template_engine: &'a mut TemplateEngine,
}

impl<'a> CommandLoader<'a> {
    pub fn new(template_engine: &'a mut TemplateEngine) -> Self {
        Self { template_engine }
    }

    /// Load all commands from the commands directory
    pub fn load_all_commands(&mut self) -> Result<HashMap<String, Command>> {
        let mut commands = HashMap::new();
        let commands_dir = Path::new("commands");

        // Commands directory is optional
        if !commands_dir.exists() {
            return Ok(commands);
        }

        // Load each command file
        for path in FileScanner::scan_directory(commands_dir)? {
            let content = std::fs::read_to_string(&path)?;
            let processed_content = self.process_file_content(&path, &content)?;

            let command: Command = serde_yaml::from_str(&processed_content)
                .with_context(|| format!("Failed to parse command file {}", path.display()))?;

            let key = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();

            if key.is_empty() {
                anyhow::bail!("Invalid command filename: {}", path.display());
            }

            if commands.contains_key(&key) {
                anyhow::bail!("Duplicate command key: {}", key);
            }

            commands.insert(key, command);
        }

        Ok(commands)
    }

    /// Process file content with templating if needed
    fn process_file_content(&mut self, path: &Path, content: &str) -> Result<String> {
        let is_template = crate::templating::TemplateEngine::is_template_file(path);
        self.template_engine
            .render_file_with_path(content, path, is_template)
            .map_err(|e| anyhow::anyhow!("{:?}", e))
    }
}
