use anyhow::Result;
use miette::Result as MietteResult;
use minijinja::{Environment, Value};
use serde_yaml::Value as YamlValue;
use std::collections::HashMap;
use std::path::Path;

use super::functions::register_functions;
use super::variables::VariableResolver;

pub struct TemplateEngine {
    env: Environment<'static>,
    resolver: VariableResolver,
}

impl Default for TemplateEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl TemplateEngine {
    pub fn new() -> Self {
        let mut env = Environment::new();

        // Configure minijinja to fail on undefined variables
        env.set_undefined_behavior(minijinja::UndefinedBehavior::Strict);

        register_functions(&mut env);

        Self {
            env,
            resolver: VariableResolver::new(),
        }
    }

    /// Add variables from various sources
    pub fn add_vars_section(&mut self, vars: &HashMap<String, YamlValue>) {
        self.resolver.add_vars_section(vars);
    }

    pub fn add_env_vars(&mut self) -> Result<()> {
        self.resolver.add_env_vars()
    }

    pub fn add_cli_vars(&mut self, cli_vars: &HashMap<String, String>) {
        self.resolver.add_cli_vars(cli_vars);
    }

    /// Render a template string with current variables
    pub fn render_string(&mut self, template: &str) -> MietteResult<String> {
        // Convert variables to MiniJinja format
        let mut context = HashMap::new();
        for (key, yaml_value) in self.resolver.get_variables() {
            // Convert YamlValue to MiniJinja Value
            let value = yaml_value_to_minijinja_value(yaml_value);
            context.insert(key.clone(), value);
        }

        let rendered = self.env.render_str(template, &context).map_err(|e| {
            // MiniJinja provides excellent error information
            let error_msg = format!("{e}");

            // Extract line/column from MiniJinja error
            let line_col = if let Some(range) = e.range() {
                // Convert byte offset to line/column
                let line = template[..range.start].lines().count();
                let col = if let Some(last_line) = template[..range.start].lines().last() {
                    last_line.len() + 1
                } else {
                    range.start + 1
                };
                Some((line, col, range.start, range.end))
            } else {
                None
            };

            // Create a helpful error message with source context
            if let Some((line, col, start, end)) = line_col {
                miette::miette! {
                    labels = vec![
                        miette::LabeledSpan::at(start..end, "error here")
                    ],
                    "Template error at line {line}, column {col}:\n{error_msg}",
                    line = line,
                    col = col,
                    error_msg = error_msg
                }
                .with_source_code(template.to_string())
            } else {
                miette::miette! {
                    "Template error:\n{error_msg}",
                    error_msg = error_msg
                }
                .with_source_code(template.to_string())
            }
        })?;
        Ok(rendered)
    }

    /// Check if a file should be treated as a template based on extension
    pub fn is_template_file(path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext == "j2")
            .unwrap_or(false)
    }

    /// Render a file, handling both .yml and .yml.j2 files
    pub fn render_file(&mut self, content: &str, is_template: bool) -> MietteResult<String> {
        if is_template {
            // Full template rendering for .j2 files
            self.render_string(content)
        } else {
            // Basic variable substitution for regular .yml files
            self.render_string(content)
        }
    }
}

/// Convert YamlValue to MiniJinja Value
fn yaml_value_to_minijinja_value(yaml_value: &YamlValue) -> Value {
    match yaml_value {
        YamlValue::Null => Value::UNDEFINED,
        YamlValue::Bool(b) => Value::from(*b),
        YamlValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::from(i)
            } else if let Some(f) = n.as_f64() {
                Value::from(f)
            } else {
                Value::UNDEFINED
            }
        }
        YamlValue::String(s) => Value::from(s.clone()),
        YamlValue::Sequence(seq) => {
            let vec: Vec<Value> = seq.iter().map(yaml_value_to_minijinja_value).collect();
            Value::from(vec)
        }
        YamlValue::Mapping(map) => {
            let mut hash = HashMap::new();
            for (k, v) in map {
                if let YamlValue::String(key) = k {
                    hash.insert(key.clone(), yaml_value_to_minijinja_value(v));
                }
            }
            Value::from_serialize(&hash)
        }
        _ => Value::UNDEFINED,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_basic_template_rendering() {
        let mut engine = TemplateEngine::new();

        // Add some variables
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), YamlValue::String("test".to_string()));
        vars.insert("version".to_string(), YamlValue::String("1.0".to_string()));
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
        vars.insert(
            "name".to_string(),
            YamlValue::String("  test  ".to_string()),
        );
        engine.add_vars_section(&vars);

        let template = "Hello {{ name | trim | upper }}";
        let result = engine.render_string(template).unwrap();
        assert_eq!(result, "Hello TEST");
    }

    #[test]
    fn test_template_with_conditionals() {
        let mut engine = TemplateEngine::new();

        let mut vars = HashMap::new();
        vars.insert("use_postgres".to_string(), YamlValue::Bool(true));
        vars.insert(
            "postgres_version".to_string(),
            YamlValue::String("16.1".to_string()),
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
        let envs = YamlValue::Sequence(vec![
            YamlValue::String("dev".to_string()),
            YamlValue::String("staging".to_string()),
            YamlValue::String("prod".to_string()),
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
        assert!(TemplateEngine::is_template_file(Path::new("config.yml.j2")));
        assert!(TemplateEngine::is_template_file(Path::new(
            "jobs/test.yaml.j2"
        )));
        assert!(!TemplateEngine::is_template_file(Path::new("config.yml")));
        assert!(!TemplateEngine::is_template_file(Path::new(
            "jobs/test.yaml"
        )));
        assert!(!TemplateEngine::is_template_file(Path::new("config")));
    }

    #[test]
    fn test_render_file_regular_yaml() {
        let mut engine = TemplateEngine::new();

        let mut vars = HashMap::new();
        vars.insert(
            "postgres_version".to_string(),
            YamlValue::String("16.1".to_string()),
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
        vars.insert("use_postgres".to_string(), YamlValue::Bool(true));
        vars.insert(
            "postgres_version".to_string(),
            YamlValue::String("16.1".to_string()),
        );
        engine.add_vars_section(&vars);

        let content = r#"
services:
{% if use_postgres %}
  postgres:
    image: postgres:{{ postgres_version }}
{% endif %}
"#;

        // Test .j2 file (should process full template logic)
        let result = engine.render_file(content, true).unwrap();
        assert!(result.contains("postgres:16.1"));
    }

    #[test]
    fn test_missing_variable_error() {
        let mut engine = TemplateEngine::new();

        let template = "Hello {{ missing_var }}";
        let result = engine.render_string(template);
        assert!(result.is_err());
    }

    #[test]
    fn test_variable_precedence_in_rendering() {
        let mut engine = TemplateEngine::new();

        // Add vars section
        let mut vars = HashMap::new();
        vars.insert(
            "test_var".to_string(),
            YamlValue::String("from_vars".to_string()),
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
