use std::collections::HashMap;
use tera::{Result as TeraResult, Value};

/// Read function for templates - reads file content relative to config directory
pub fn read_function(args: &HashMap<String, Value>) -> TeraResult<Value> {
    let filename = args
        .get("filename")
        .and_then(|v| v.as_str())
        .ok_or_else(|| tera::Error::msg("read() function requires a filename argument"))?;

    // TODO: Pass config directory path from context
    let config_dir = std::env::current_dir().unwrap();
    let file_path = config_dir.join(filename);

    match std::fs::read_to_string(&file_path) {
        Ok(content) => Ok(Value::String(content)),
        Err(e) => Err(tera::Error::msg(format!(
            "Failed to read file '{}': {}",
            file_path.display(),
            e
        ))),
    }
}

/// Register all custom functions with Tera
pub fn register_functions(tera: &mut tera::Tera) {
    tera.register_function("read", read_function);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::fs;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_read_function_success() {
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
        let mut args = HashMap::new();
        args.insert(
            "filename".to_string(),
            Value::String("test.txt".to_string()),
        );

        let result = read_function(&args).unwrap();
        if let Value::String(content) = result {
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
        let temp_dir = tempdir().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&temp_dir).unwrap();

        let mut args = HashMap::new();
        args.insert(
            "filename".to_string(),
            Value::String("nonexistent.txt".to_string()),
        );

        let result = read_function(&args);
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
    fn test_read_function_no_filename() {
        let args = HashMap::new();

        let result = read_function(&args);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "read() function requires a filename argument"
        );
    }

    #[test]
    fn test_read_function_invalid_filename() {
        let mut args = HashMap::new();
        args.insert("filename".to_string(), Value::Number(42.into()));

        let result = read_function(&args);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "read() function requires a filename argument"
        );
    }

    #[test]
    fn test_read_function_in_template() {
        let temp_dir = tempdir().unwrap();
        let temp_file = temp_dir.path().join("script.sh");

        // Create a test script file
        let mut file = fs::File::create(&temp_file).unwrap();
        writeln!(file, "#!/bin/bash").unwrap();
        writeln!(file, "echo 'Setting up environment'").unwrap();
        writeln!(file, "npm install").unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&temp_dir).unwrap();

        // Test with Tera template engine
        let mut tera =
            tera::Tera::new("templates/**/*").unwrap_or_else(|_| tera::Tera::new("").unwrap());
        register_functions(&mut tera);

        let template = r#"
steps:
  - run: |
      {{ read(filename="script.sh") | trim }}
"#;

        let context = tera::Context::new();
        let result = tera.render_str(template, &context).unwrap();

        assert!(result.contains("#!/bin/bash"));
        assert!(result.contains("echo 'Setting up environment'"));
        assert!(result.contains("npm install"));

        std::env::set_current_dir(original_dir).unwrap();
    }
}
