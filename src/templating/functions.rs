use minijinja::{Environment, Error, Value};

/// Read function for templates - reads file content relative to config directory
pub fn read_function(filename: Value) -> Result<Value, Error> {
    let filename = filename.as_str().ok_or_else(|| {
        Error::new(
            minijinja::ErrorKind::InvalidOperation,
            "read() function requires a filename string",
        )
    })?;

    // TODO: Pass config directory path from context
    let config_dir = std::env::current_dir().map_err(|e| {
        Error::new(
            minijinja::ErrorKind::InvalidOperation,
            format!("Failed to get current directory: {e}"),
        )
    })?;
    let file_path = config_dir.join(filename);

    match std::fs::read_to_string(&file_path) {
        Ok(content) => Ok(Value::from(content)),
        Err(e) => Err(Error::new(
            minijinja::ErrorKind::InvalidOperation,
            format!("Failed to read file '{}': {}", file_path.display(), e),
        )),
    }
}

/// Register all custom functions with MiniJinja
pub fn register_functions(env: &mut Environment) {
    env.add_function("read", read_function);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use std::sync::Mutex;
    use tempfile::tempdir;

    // Shared mutex to serialize directory changes across all tests
    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_read_function_success() {
        let _guard = TEST_MUTEX.lock().unwrap();

        let temp_dir = tempdir().unwrap();
        let temp_file = temp_dir.path().join("test.txt");

        // Create a test file
        let mut file = fs::File::create(&temp_file).unwrap();
        writeln!(file, "Hello, World!").unwrap();
        writeln!(file, "This is a test file.").unwrap();

        // Change to temp directory
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&temp_dir).unwrap();

        // Test the read function
        let result = read_function(Value::from("test.txt")).unwrap();
        if let Some(content) = result.as_str() {
            assert!(content.contains("Hello, World!"));
            assert!(content.contains("This is a test file."));
        } else {
            panic!("Expected string value");
        }

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_read_function_missing_file() {
        let _guard = TEST_MUTEX.lock().unwrap();

        let temp_dir = tempdir().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&temp_dir).unwrap();

        let result = read_function(Value::from("nonexistent.txt"));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Failed to read file")
        );

        std::env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_read_function_invalid_arg() {
        let result = read_function(Value::from(42));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("requires a filename string")
        );
    }

    #[test]
    fn test_read_function_in_template() {
        let _guard = TEST_MUTEX.lock().unwrap();

        let temp_dir = tempdir().unwrap();
        let temp_file = temp_dir.path().join("script.sh");

        // Create a test script file
        let mut file = fs::File::create(&temp_file).unwrap();
        writeln!(file, "#!/bin/bash").unwrap();
        writeln!(file, "echo 'Setting up environment'").unwrap();
        writeln!(file, "npm install").unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&temp_dir).unwrap();

        // Test with MiniJinja template engine
        let mut env = Environment::new();
        register_functions(&mut env);

        let template = r#"
steps:
  - run: |
      {{ read('script.sh') | trim }}
"#;

        let result = env.render_str(template, ()).unwrap();

        assert!(result.contains("#!/bin/bash"));
        assert!(result.contains("echo 'Setting up environment'"));
        assert!(result.contains("npm install"));

        std::env::set_current_dir(original_dir).unwrap();
    }
}
