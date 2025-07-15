use anyhow::Result;
use serde_yaml::Value;
use std::collections::HashMap;
use std::env;

pub struct VariableResolver {
    variables: HashMap<String, Value>,
}

impl Default for VariableResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl VariableResolver {
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
        }
    }

    /// Add variables from a vars section in config
    pub fn add_vars_section(&mut self, vars: &HashMap<String, Value>) {
        for (key, value) in vars {
            self.variables.insert(key.clone(), value.clone());
        }
    }

    /// Add variables from environment (CIGEN_VAR_* -> variable name)
    pub fn add_env_vars(&mut self) -> Result<()> {
        for (key, value) in env::vars() {
            if let Some(var_name) = key.strip_prefix("CIGEN_VAR_") {
                let var_name = var_name.to_lowercase();
                self.variables.insert(var_name, Value::String(value));
            }
        }
        Ok(())
    }

    /// Add variables from CLI flags
    pub fn add_cli_vars(&mut self, cli_vars: &HashMap<String, String>) {
        for (key, value) in cli_vars {
            self.variables
                .insert(key.clone(), Value::String(value.clone()));
        }
    }

    /// Get all variables as a HashMap for MiniJinja templates
    pub fn get_variables(&self) -> &HashMap<String, Value> {
        &self.variables
    }

    /// Get a specific variable
    pub fn get_variable(&self, name: &str) -> Option<&Value> {
        self.variables.get(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_variable_precedence() {
        let mut resolver = VariableResolver::new();

        // 1. Add vars section (lowest precedence)
        let mut vars_section = HashMap::new();
        vars_section.insert(
            "test_var".to_string(),
            Value::String("from_vars".to_string()),
        );
        vars_section.insert(
            "only_in_vars".to_string(),
            Value::String("vars_only".to_string()),
        );
        resolver.add_vars_section(&vars_section);

        // 2. Add CLI vars (highest precedence)
        let mut cli_vars = HashMap::new();
        cli_vars.insert("test_var".to_string(), "from_cli".to_string());
        cli_vars.insert("only_in_cli".to_string(), "cli_only".to_string());
        resolver.add_cli_vars(&cli_vars);

        // Test precedence: CLI should override vars
        assert_eq!(
            resolver.get_variable("test_var"),
            Some(&Value::String("from_cli".to_string()))
        );

        // Test unique vars
        assert_eq!(
            resolver.get_variable("only_in_vars"),
            Some(&Value::String("vars_only".to_string()))
        );
        assert_eq!(
            resolver.get_variable("only_in_cli"),
            Some(&Value::String("cli_only".to_string()))
        );
    }

    #[test]
    fn test_env_vars_precedence() {
        let mut resolver = VariableResolver::new();

        // 1. Add vars section
        let mut vars_section = HashMap::new();
        vars_section.insert(
            "shared_var".to_string(),
            Value::String("from_vars".to_string()),
        );
        resolver.add_vars_section(&vars_section);

        // 2. Set environment variable
        unsafe {
            env::set_var("CIGEN_VAR_SHARED_VAR", "from_env");
            env::set_var("CIGEN_VAR_ENV_ONLY", "env_only");
        }

        // Add env vars
        resolver.add_env_vars().unwrap();

        // Test: env should override vars
        assert_eq!(
            resolver.get_variable("shared_var"),
            Some(&Value::String("from_env".to_string()))
        );

        // Test env-only var
        assert_eq!(
            resolver.get_variable("env_only"),
            Some(&Value::String("env_only".to_string()))
        );

        // 3. Add CLI vars (should override env)
        let mut cli_vars = HashMap::new();
        cli_vars.insert("shared_var".to_string(), "from_cli".to_string());
        resolver.add_cli_vars(&cli_vars);

        // Test: CLI should override env
        assert_eq!(
            resolver.get_variable("shared_var"),
            Some(&Value::String("from_cli".to_string()))
        );

        // Cleanup
        unsafe {
            env::remove_var("CIGEN_VAR_SHARED_VAR");
            env::remove_var("CIGEN_VAR_ENV_ONLY");
        }
    }

    #[test]
    fn test_env_var_name_conversion() {
        let mut resolver = VariableResolver::new();

        // Set various env vars
        unsafe {
            env::set_var("CIGEN_VAR_POSTGRES_VERSION", "16.1");
            env::set_var("CIGEN_VAR_REDIS_VERSION", "7.4.0");
            env::set_var("NOT_CIGEN_VAR", "should_be_ignored");
        }

        resolver.add_env_vars().unwrap();

        // Test conversion from CIGEN_VAR_* to lowercase
        assert_eq!(
            resolver.get_variable("postgres_version"),
            Some(&Value::String("16.1".to_string()))
        );
        assert_eq!(
            resolver.get_variable("redis_version"),
            Some(&Value::String("7.4.0".to_string()))
        );

        // Test that non-CIGEN_VAR_ variables are ignored
        assert_eq!(resolver.get_variable("not_cigen_var"), None);

        // Cleanup
        unsafe {
            env::remove_var("CIGEN_VAR_POSTGRES_VERSION");
            env::remove_var("CIGEN_VAR_REDIS_VERSION");
            env::remove_var("NOT_CIGEN_VAR");
        }
    }

    #[test]
    fn test_empty_resolver() {
        let resolver = VariableResolver::new();
        assert_eq!(resolver.get_variable("nonexistent"), None);
        assert!(resolver.get_variables().is_empty());
    }
}
