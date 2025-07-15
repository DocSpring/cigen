use anyhow::Result;
use serde_yaml::Value;
use std::collections::HashMap;
use std::path::Path;
use tera::{Context, Tera};

use super::functions::register_functions;
use super::variables::VariableResolver;

pub struct TemplateEngine {
    tera: Tera,
    resolver: VariableResolver,
}

impl Default for TemplateEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl TemplateEngine {
    pub fn new() -> Self {
        let mut tera = Tera::new("templates/**/*").unwrap_or_else(|_| Tera::new("").unwrap());
        register_functions(&mut tera);

        Self {
            tera,
            resolver: VariableResolver::new(),
        }
    }

    /// Add variables from various sources
    pub fn add_vars_section(&mut self, vars: &HashMap<String, Value>) {
        self.resolver.add_vars_section(vars);
    }

    pub fn add_env_vars(&mut self) -> Result<()> {
        self.resolver.add_env_vars()
    }

    pub fn add_cli_vars(&mut self, cli_vars: &HashMap<String, String>) {
        self.resolver.add_cli_vars(cli_vars);
    }

    /// Render a template string with current variables
    pub fn render_string(&mut self, template: &str) -> Result<String> {
        let mut context = Context::new();

        // Add all variables to context
        for (key, value) in self.resolver.get_variables() {
            context.insert(key, value);
        }

        let rendered = self
            .tera
            .render_str(template, &context)
            .map_err(|e| anyhow::anyhow!("Template rendering error: {}", e))?;
        Ok(rendered)
    }

    /// Check if a file should be treated as a template based on extension
    pub fn is_template_file(path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext == "tera")
            .unwrap_or(false)
    }

    /// Render a file, handling both .yml and .yml.tera files
    pub fn render_file(&mut self, content: &str, is_template: bool) -> Result<String> {
        if is_template {
            // Full template rendering for .tera files
            self.render_string(content)
        } else {
            // Basic variable substitution for regular .yml files
            self.render_string(content)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    #[test]
    fn test_basic_template_rendering() {
        let mut engine = TemplateEngine::new();

        // Add some variables
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), Value::String("test".to_string()));
        vars.insert("version".to_string(), Value::String("1.0".to_string()));
        engine.add_vars_section(&vars);

        // Test basic variable substitution
        let template = "Hello {{ name }}, version {{ version }}";
        let result = engine.render_string(template).unwrap();
        assert_eq!(result, "Hello test, version 1.0");
    }

    #[test]
    fn test_template_with_filters() {
        let mut engine = TemplateEngine::new();

        let mut vars = HashMap::new();
        vars.insert("name".to_string(), Value::String("  test  ".to_string()));
        engine.add_vars_section(&vars);

        let template = "Hello {{ name | trim | upper }}";
        let result = engine.render_string(template).unwrap();
        assert_eq!(result, "Hello TEST");
    }

    #[test]
    fn test_template_with_conditionals() {
        let mut engine = TemplateEngine::new();

        let mut vars = HashMap::new();
        vars.insert("use_postgres".to_string(), Value::Bool(true));
        vars.insert(
            "postgres_version".to_string(),
            Value::String("16.1".to_string()),
        );
        engine.add_vars_section(&vars);

        let template = r#"
services:
{% if use_postgres %}
  postgres:
    image: postgres:{{ postgres_version }}
{% endif %}
"#;
        let result = engine.render_string(template).unwrap();
        assert!(result.contains("postgres:16.1"));
    }

    #[test]
    fn test_template_with_loops() {
        let mut engine = TemplateEngine::new();

        let mut vars = HashMap::new();
        let envs = Value::Sequence(vec![
            Value::String("dev".to_string()),
            Value::String("staging".to_string()),
            Value::String("prod".to_string()),
        ]);
        vars.insert("environments".to_string(), envs);
        engine.add_vars_section(&vars);

        let template = r#"
{% for env in environments %}
deploy_{{ env }}:
  image: myapp:latest
{% endfor %}
"#;
        let result = engine.render_string(template).unwrap();
        assert!(result.contains("deploy_dev:"));
        assert!(result.contains("deploy_staging:"));
        assert!(result.contains("deploy_prod:"));
    }

    #[test]
    fn test_is_template_file() {
        assert!(TemplateEngine::is_template_file(&PathBuf::from(
            "config.yml.tera"
        )));
        assert!(TemplateEngine::is_template_file(&PathBuf::from(
            "jobs/test.yaml.tera"
        )));
        assert!(!TemplateEngine::is_template_file(&PathBuf::from(
            "config.yml"
        )));
        assert!(!TemplateEngine::is_template_file(&PathBuf::from(
            "jobs/test.yaml"
        )));
        assert!(!TemplateEngine::is_template_file(&PathBuf::from("config")));
    }

    #[test]
    fn test_render_file_regular_yaml() {
        let mut engine = TemplateEngine::new();

        let mut vars = HashMap::new();
        vars.insert(
            "postgres_version".to_string(),
            Value::String("16.1".to_string()),
        );
        engine.add_vars_section(&vars);

        let content = r#"
services:
  postgres:
    image: postgres:{{ postgres_version }}
"#;

        // Test regular .yml file (should still process variables)
        let result = engine.render_file(content, false).unwrap();
        assert!(result.contains("postgres:16.1"));
    }

    #[test]
    fn test_render_file_template() {
        let mut engine = TemplateEngine::new();

        let mut vars = HashMap::new();
        vars.insert("use_postgres".to_string(), Value::Bool(true));
        vars.insert(
            "postgres_version".to_string(),
            Value::String("16.1".to_string()),
        );
        engine.add_vars_section(&vars);

        let content = r#"
services:
{% if use_postgres %}
  postgres:
    image: postgres:{{ postgres_version }}
{% endif %}
"#;

        // Test .tera file (should process full template logic)
        let result = engine.render_file(content, true).unwrap();
        assert!(result.contains("postgres:16.1"));
    }

    #[test]
    fn test_missing_variable_error() {
        let mut engine = TemplateEngine::new();

        let template = "Hello {{ missing_var }}";
        let result = engine.render_string(template);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Template rendering error")
        );
    }

    #[test]
    fn test_variable_precedence_in_rendering() {
        let mut engine = TemplateEngine::new();

        // Add vars section
        let mut vars = HashMap::new();
        vars.insert(
            "test_var".to_string(),
            Value::String("from_vars".to_string()),
        );
        engine.add_vars_section(&vars);

        // Add CLI vars (should override)
        let mut cli_vars = HashMap::new();
        cli_vars.insert("test_var".to_string(), "from_cli".to_string());
        engine.add_cli_vars(&cli_vars);

        let template = "Value: {{ test_var }}";
        let result = engine.render_string(template).unwrap();
        assert_eq!(result, "Value: from_cli");
    }
}
