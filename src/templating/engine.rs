use anyhow::Result;
use miette::Result as MietteResult;
use minijinja::{Environment, Value};
use serde_yaml::Value as YamlValue;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::error::TemplateError;
use super::functions::register_functions;
use super::variables::VariableResolver;

pub struct TemplateEngine {
    env: Environment<'static>,
    resolver: VariableResolver,
    current_file: Option<PathBuf>,
    template_base: Option<PathBuf>,
    templates: HashMap<String, String>,
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
            template_base: None,
            templates: HashMap::new(),
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

    /// Set the base directory for template resolution
    pub fn set_template_base(&mut self, base_dir: &Path) -> Result<()> {
        self.template_base = Some(base_dir.to_path_buf());
        self.load_templates_recursive(base_dir, base_dir)?;
        self.setup_template_loader()?;
        Ok(())
    }

    /// Set up the template loader for includes
    fn setup_template_loader(&mut self) -> Result<()> {
        // The include functionality needs to be handled differently
        // Since minijinja doesn't have set_loader, we'll use a different approach
        // For now, we'll need to implement this at render time
        Ok(())
    }

    /// Recursively load all .j2 templates from the base directory
    fn load_templates_recursive(&mut self, current_dir: &Path, base_dir: &Path) -> Result<()> {
        if !current_dir.is_dir() {
            return Ok(());
        }

        for entry in fs::read_dir(current_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                self.load_templates_recursive(&path, base_dir)?;
            } else if Self::is_template_file(&path) {
                let content = fs::read_to_string(&path)?;

                // Get relative path from base directory
                let relative_path = path
                    .strip_prefix(base_dir)
                    .map_err(|_| anyhow::anyhow!("Failed to get relative path for template"))?;

                // Use the path without .j2 extension as the template name
                let template_name = if let Some(stem) = relative_path.file_stem() {
                    relative_path.with_file_name(stem)
                } else {
                    relative_path.to_path_buf()
                };

                let template_name_str = template_name.to_string_lossy().into_owned();
                self.templates.insert(template_name_str, content);
            }
        }

        Ok(())
    }

    /// Preprocess template to resolve includes
    fn preprocess_includes(&self, template: &str) -> Result<String> {
        use regex::Regex;

        // Match {% include "template_name" %} patterns
        let include_regex = Regex::new(r#"\{\%\s*include\s+"([^"]+)"\s*\%\}"#)?;
        let mut result = template.to_string();

        // Keep processing until no more includes are found (handles nested includes)
        let mut max_depth = 10; // Prevent infinite loops
        while max_depth > 0 && include_regex.is_match(&result) {
            let new_result = include_regex
                .replace_all(&result, |caps: &regex::Captures| {
                    let template_name = &caps[1];

                    // Look up the template content
                    if let Some(content) = self.templates.get(template_name) {
                        content.clone()
                    } else {
                        // If template not found, leave the include tag as-is and let minijinja handle the error
                        caps[0].to_string()
                    }
                })
                .to_string();

            if new_result == result {
                break; // No changes made, stop processing
            }

            result = new_result;
            max_depth -= 1;
        }

        Ok(result)
    }

    /// Set the current file being processed for better error messages
    pub fn set_current_file(&mut self, path: &Path) {
        self.current_file = Some(path.to_path_buf());
    }

    /// Clear the current file
    pub fn clear_current_file(&mut self) {
        self.current_file = None;
    }

    /// Render a string with only simple variable substitution (no complex template features)
    /// This is used for regular YAML files that may contain target CI system template syntax
    fn render_string_variables_only(&mut self, content: &str) -> MietteResult<String> {
        // Simple regex-based variable substitution to avoid complex template processing
        // This handles {{ variable_name }} patterns but skips complex Liquid syntax
        let mut result = content.to_string();

        // Match simple variable patterns like {{ variable_name }} (no filters or complex expressions)
        let re = regex::Regex::new(r"\{\{\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*\}\}")
            .map_err(|e| miette::miette!("Regex compilation error: {}", e))?;

        for captures in re.captures_iter(content) {
            if let Some(var_match) = captures.get(0)
                && let Some(var_name) = captures.get(1)
            {
                let var_name_str = var_name.as_str();
                if let Some(value) = self.get_variable_value(var_name_str) {
                    result = result.replace(var_match.as_str(), &value);
                }
            }
        }

        Ok(result)
    }

    /// Get variable value as string from current context
    fn get_variable_value(&self, var_name: &str) -> Option<String> {
        // Get from variable resolver
        if let Some(value) = self.resolver.get_variable(var_name) {
            match value {
                serde_yaml::Value::String(s) => Some(s.clone()),
                serde_yaml::Value::Number(n) => Some(n.to_string()),
                serde_yaml::Value::Bool(b) => Some(b.to_string()),
                _ => None,
            }
        } else {
            None
        }
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

        // Preprocess includes if template_base is set
        let processed_template = if self.template_base.is_some() {
            self.preprocess_includes(template)
                .map_err(|e| miette::miette!("Include preprocessing error: {}", e))?
        } else {
            template.to_string()
        };

        // Render the template
        match self.env.render_str(&processed_template, &context) {
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
    pub fn render_file(&mut self, content: &str, is_template: bool) -> MietteResult<String> {
        if is_template {
            // Template files get full template processing
            self.render_string(content)
        } else {
            // Regular YAML files still get variable substitution but not complex template processing
            // This allows {{ postgres_version }} to work but prevents complex Liquid syntax from failing
            self.render_string_variables_only(content)
        }
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

    /// Render a named template from the loaded template source
    pub fn render_template(
        &mut self,
        template_name: &str,
        context: &HashMap<String, serde_json::Value>,
    ) -> Result<String> {
        // Get the template content from our stored templates
        let template_content = self
            .templates
            .get(template_name)
            .ok_or_else(|| anyhow::anyhow!("Template '{}' not found", template_name))?;

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

        // Preprocess includes in the template
        let processed_template = self
            .preprocess_includes(template_content)
            .map_err(|e| anyhow::anyhow!("Include preprocessing error: {}", e))?;

        // Render the template
        self.env
            .render_str(&processed_template, &minijinja_context)
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

    #[test]
    fn test_template_includes() {
        use std::fs;
        use tempfile::TempDir;

        // Create a temporary directory for templates
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Create includes directory
        let includes_dir = base_path.join("includes");
        fs::create_dir(&includes_dir).unwrap();

        // Create a partial template
        let partial_content = r#"version: {{ version }}
image: ruby:{{ ruby_version }}"#;
        fs::write(includes_dir.join("common.j2"), partial_content).unwrap();

        // Create main template
        let main_template = r#"name: test-app
{% include "includes/common" %}
environment: production"#;

        // Set up the template engine
        let mut engine = TemplateEngine::new();
        engine.set_template_base(base_path).unwrap();

        let mut vars = HashMap::new();
        vars.insert(
            "version".to_string(),
            YamlValue::String("1.0.0".to_string()),
        );
        vars.insert(
            "ruby_version".to_string(),
            YamlValue::String("3.3.0".to_string()),
        );
        engine.add_vars_section(&vars);

        // Render the template
        let result = engine.render_string(main_template).unwrap();

        assert!(result.contains("name: test-app"));
        assert!(result.contains("version: 1.0.0"));
        assert!(result.contains("image: ruby:3.3.0"));
        assert!(result.contains("environment: production"));
    }

    #[test]
    fn test_nested_template_includes() {
        use std::fs;
        use tempfile::TempDir;

        // Create a temporary directory for templates
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Create nested directories
        let includes_dir = base_path.join("includes");
        let docker_dir = includes_dir.join("docker");
        fs::create_dir_all(&docker_dir).unwrap();

        // Create nested partial templates
        let base_partial = r#"base_image: ubuntu:{{ ubuntu_version }}"#;
        fs::write(docker_dir.join("base.j2"), base_partial).unwrap();

        let ruby_partial = r#"{% include "includes/docker/base" %}
ruby_image: ruby:{{ ruby_version }}"#;
        fs::write(includes_dir.join("ruby.j2"), ruby_partial).unwrap();

        // Create main template
        let main_template = r#"name: my-app
{% include "includes/ruby" %}
environment: {{ env }}"#;

        // Set up the template engine
        let mut engine = TemplateEngine::new();
        engine.set_template_base(base_path).unwrap();

        let mut vars = HashMap::new();
        vars.insert(
            "ubuntu_version".to_string(),
            YamlValue::String("22.04".to_string()),
        );
        vars.insert(
            "ruby_version".to_string(),
            YamlValue::String("3.3.0".to_string()),
        );
        vars.insert(
            "env".to_string(),
            YamlValue::String("production".to_string()),
        );
        engine.add_vars_section(&vars);

        // Render the template
        let result = engine.render_string(main_template).unwrap();

        assert!(result.contains("name: my-app"));
        assert!(result.contains("base_image: ubuntu:22.04"));
        assert!(result.contains("ruby_image: ruby:3.3.0"));
        assert!(result.contains("environment: production"));
    }

    #[test]
    fn test_render_named_template() {
        use std::fs;
        use tempfile::TempDir;

        // Create a temporary directory for templates
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Create a named template
        let template_content = r#"job_name: {{ job_name }}
image: {{ image }}
steps:
{% for step in steps %}
  - {{ step }}
{% endfor %}"#;
        fs::write(base_path.join("job.j2"), template_content).unwrap();

        // Set up the template engine
        let mut engine = TemplateEngine::new();
        engine.set_template_base(base_path).unwrap();

        // Create context
        let mut context = HashMap::new();
        context.insert(
            "job_name".to_string(),
            serde_json::Value::String("test-job".to_string()),
        );
        context.insert(
            "image".to_string(),
            serde_json::Value::String("ruby:3.3".to_string()),
        );
        context.insert(
            "steps".to_string(),
            serde_json::Value::Array(vec![
                serde_json::Value::String("checkout".to_string()),
                serde_json::Value::String("bundle install".to_string()),
                serde_json::Value::String("rspec".to_string()),
            ]),
        );

        // Render the named template
        let result = engine.render_template("job", &context).unwrap();

        assert!(result.contains("job_name: test-job"));
        assert!(result.contains("image: ruby:3.3"));
        assert!(result.contains("- checkout"));
        assert!(result.contains("- bundle install"));
        assert!(result.contains("- rspec"));
    }
}
