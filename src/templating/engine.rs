use anyhow::Result;
use miette::Result as MietteResult;
use minijinja::{Environment, Value};
use serde_yaml::Value as YamlValue;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::error::TemplateError;
use super::functions::register_functions;
use super::variables::VariableResolver;

pub struct TemplateEngine {
    env: Environment<'static>,
    resolver: VariableResolver,
    current_file: Option<PathBuf>,
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
            current_file: None,
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

    /// Set the current file being processed for better error messages
    pub fn set_current_file(&mut self, path: &Path) {
        self.current_file = Some(path.to_path_buf());
    }

    /// Clear the current file
    pub fn clear_current_file(&mut self) {
        self.current_file = None;
    }

    /// Render a template string with current variables
    pub fn render_string(&mut self, template: &str) -> MietteResult<String> {
        self.render_string_with_path(template, None)
    }

    /// Render a template string with a specific source path
    pub fn render_string_with_path(
        &mut self,
        template: &str,
        path: Option<&Path>,
    ) -> MietteResult<String> {
        // Convert variables to MiniJinja format
        let mut context = HashMap::new();
        for (key, yaml_value) in self.resolver.get_variables() {
            // Convert YamlValue to MiniJinja Value
            let value = yaml_value_to_minijinja_value(yaml_value);
            context.insert(key.clone(), value);
        }

        // Render the template
        match self.env.render_str(template, &context) {
            Ok(rendered) => Ok(rendered),
            Err(error) => {
                // Determine the source path
                let source_path = path
                    .or(self.current_file.as_deref())
                    .unwrap_or(Path::new("<inline>"));

                // Create a miette-compatible error
                let template_error =
                    TemplateError::from_minijinja_error(error, template.to_string(), source_path);

                Err(template_error.into())
            }
        }
    }

    /// Check if a file is a template file based on extension
    pub fn is_template_file(path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext == "j2")
            .unwrap_or(false)
    }

    /// Render a file, handling both .yml and .yml.j2 files
    pub fn render_file(&mut self, content: &str, _is_template: bool) -> MietteResult<String> {
        // Both file types get full template processing now
        self.render_string(content)
    }

    /// Render a file with a specific path
    pub fn render_file_with_path(
        &mut self,
        content: &str,
        path: &Path,
        is_template: bool,
    ) -> MietteResult<String> {
        self.set_current_file(path);
        let result = self.render_file(content, is_template);
        self.clear_current_file();
        result
    }

    /// Render a template string with a provided context
    pub fn render_str(
        &mut self,
        template: &str,
        context: &HashMap<String, serde_json::Value>,
    ) -> Result<String> {
        // Convert serde_json::Value to minijinja::Value
        let mut minijinja_context = HashMap::new();
        for (key, json_value) in context {
            let value = json_value_to_minijinja_value(json_value);
            minijinja_context.insert(key.clone(), value);
        }

        // Also add variables from the resolver
        for (key, yaml_value) in self.resolver.get_variables() {
            if !minijinja_context.contains_key(key) {
                let value = yaml_value_to_minijinja_value(yaml_value);
                minijinja_context.insert(key.clone(), value);
            }
        }

        // Render the template
        self.env
            .render_str(template, &minijinja_context)
            .map_err(|e| anyhow::anyhow!("Template rendering error: {}", e))
    }
}

/// Convert a serde_json::Value to a minijinja::Value
fn json_value_to_minijinja_value(json_value: &serde_json::Value) -> Value {
    match json_value {
        serde_json::Value::Null => Value::from_serialize(()),
        serde_json::Value::Bool(b) => Value::from(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::from(i)
            } else if let Some(f) = n.as_f64() {
                Value::from(f)
            } else {
                Value::from_serialize(n)
            }
        }
        serde_json::Value::String(s) => Value::from(s.as_str()),
        serde_json::Value::Array(arr) => Value::from_serialize(
            arr.iter()
                .map(json_value_to_minijinja_value)
                .collect::<Vec<_>>(),
        ),
        serde_json::Value::Object(obj) => {
            let converted: HashMap<String, Value> = obj
                .iter()
                .map(|(k, v)| (k.clone(), json_value_to_minijinja_value(v)))
                .collect();
            Value::from_serialize(converted)
        }
    }
}

/// Convert a serde_yaml::Value to a minijinja::Value
fn yaml_value_to_minijinja_value(yaml_value: &YamlValue) -> Value {
    match yaml_value {
        YamlValue::Null => Value::from_serialize(()),
        YamlValue::Bool(b) => Value::from(*b),
        YamlValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::from(i)
            } else if let Some(f) = n.as_f64() {
                Value::from(f)
            } else {
                Value::from_serialize(n)
            }
        }
        YamlValue::String(s) => Value::from(s.as_str()),
        YamlValue::Sequence(seq) => Value::from_serialize(
            seq.iter()
                .map(yaml_value_to_minijinja_value)
                .collect::<Vec<_>>(),
        ),
        YamlValue::Mapping(map) => {
            let converted: HashMap<String, Value> = map
                .iter()
                .filter_map(|(k, v)| {
                    k.as_str()
                        .map(|key| (key.to_string(), yaml_value_to_minijinja_value(v)))
                })
                .collect();
            Value::from_serialize(&converted)
        }
        YamlValue::Tagged(_) => Value::from_serialize(yaml_value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_template_rendering() {
        let mut engine = TemplateEngine::new();

        // Add some variables
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), YamlValue::String("World".to_string()));
        engine.add_vars_section(&vars);

        let template = "Hello {{ name }}!";
        let result = engine.render_string(template).unwrap();
        assert_eq!(result, "Hello World!");
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
{% if use_postgres %}
services:
  - postgres:{{ postgres_version }}
{% endif %}"#;

        let result = engine.render_string(template).unwrap();
        assert!(result.contains("postgres:16.1"));
    }

    #[test]
    fn test_template_with_loops() {
        let mut engine = TemplateEngine::new();

        let mut vars = HashMap::new();
        let environments = vec![
            YamlValue::String("dev".to_string()),
            YamlValue::String("staging".to_string()),
            YamlValue::String("prod".to_string()),
        ];
        vars.insert(
            "environments".to_string(),
            YamlValue::Sequence(environments),
        );
        engine.add_vars_section(&vars);

        let template = r#"{% for env in environments %}
deploy_{{ env }}:
  stage: deploy
  environment: {{ env }}
{% endfor %}"#;

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
{% if use_postgres %}
services:
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
            "version".to_string(),
            YamlValue::String("1.0.0".to_string()),
        );
        engine.add_vars_section(&vars);

        // Add CLI vars (should override)
        let mut cli_vars = HashMap::new();
        cli_vars.insert("version".to_string(), "2.0.0".to_string());
        engine.add_cli_vars(&cli_vars);

        let template = "Version: {{ version }}";
        let result = engine.render_string(template).unwrap();
        assert_eq!(result, "Version: 2.0.0");
    }

    #[test]
    fn test_template_with_filters() {
        let mut engine = TemplateEngine::new();

        let mut vars = HashMap::new();
        vars.insert("name".to_string(), YamlValue::String("world".to_string()));
        engine.add_vars_section(&vars);

        let template = "Hello {{ name | upper }}!";
        let result = engine.render_string(template).unwrap();
        assert_eq!(result, "Hello WORLD!");
    }
}
