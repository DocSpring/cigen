use minijinja::{Environment, Error, Value};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

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

/// Checksum function for templates - calculates SHA256 hash of file/directory content
pub fn checksum_function(path: Value) -> Result<Value, Error> {
    let path_str = path.as_str().ok_or_else(|| {
        Error::new(
            minijinja::ErrorKind::InvalidOperation,
            "checksum() function requires a path string",
        )
    })?;

    // For checksum, we want to resolve paths relative to the project root,
    // not the .cigen directory. If we're in .cigen, go up one level.
    let current_dir = std::env::current_dir().map_err(|e| {
        Error::new(
            minijinja::ErrorKind::InvalidOperation,
            format!("Failed to get current directory: {e}"),
        )
    })?;

    let base_dir = if current_dir.file_name() == Some(std::ffi::OsStr::new(".cigen")) {
        current_dir.parent().unwrap_or(&current_dir).to_path_buf()
    } else {
        current_dir
    };

    let path = base_dir.join(path_str);

    let mut hasher = Sha256::new();

    if path.is_file() {
        // Hash single file
        match std::fs::read(&path) {
            Ok(content) => {
                hasher.update(&content);
            }
            Err(e) => {
                return Err(Error::new(
                    minijinja::ErrorKind::InvalidOperation,
                    format!("Failed to read file '{}': {}", path.display(), e),
                ));
            }
        }
    } else if path.is_dir() {
        // Hash directory contents recursively
        let mut paths = Vec::new();
        collect_paths(&path, &mut paths).map_err(|e| {
            Error::new(
                minijinja::ErrorKind::InvalidOperation,
                format!("Failed to read directory '{}': {}", path.display(), e),
            )
        })?;

        // Sort paths for consistent hashing
        paths.sort();

        for file_path in paths {
            if let Ok(content) = std::fs::read(&file_path) {
                // Include relative path in hash for context
                if let Ok(rel_path) = file_path.strip_prefix(&base_dir) {
                    hasher.update(rel_path.to_string_lossy().as_bytes());
                }
                hasher.update(&content);
            }
        }
    } else {
        return Err(Error::new(
            minijinja::ErrorKind::InvalidOperation,
            format!(
                "Path '{}' is neither a file nor a directory",
                path.display()
            ),
        ));
    }

    let result = hasher.finalize();
    let hex_string = format!("{result:x}");
    Ok(Value::from(hex_string))
}

/// Recursively collect all file paths in a directory
fn collect_paths(dir: &PathBuf, paths: &mut Vec<PathBuf>) -> Result<(), std::io::Error> {
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                collect_paths(&path, paths)?;
            } else {
                paths.push(path);
            }
        }
    }
    Ok(())
}

/// Register all custom functions with MiniJinja
pub fn register_functions(env: &mut Environment) {
    env.add_function("read", read_function);
    env.add_function("checksum", checksum_function);
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
