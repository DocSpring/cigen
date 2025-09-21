use once_cell::sync::Lazy;
use serde_yaml::Value;
use std::collections::HashMap;

/// Built-in template commands that cigen provides
pub static TEMPLATE_COMMANDS: Lazy<HashMap<String, Value>> = Lazy::new(|| {
    let mut commands = HashMap::new();

    // continue_circleci_pipeline command
    let continue_pipeline = serde_yaml::from_str(
        r#"
description: "Continue a CircleCI pipeline with a dynamic configuration"
parameters:
  config_path:
    type: string
    description: "Path to the configuration file to continue with"
    default: ".circleci/dynamic_config.yml"
steps:
  - run:
      name: Continue Pipeline
      environment:
        CONFIG_PATH: << parameters.config_path >>
      command: |
        # Continue with pipeline
        if [ -z "${CIRCLE_CONTINUATION_KEY}" ]; then
            echo "CIRCLE_CONTINUATION_KEY is required. Make sure setup workflows are enabled."
            exit 1
        fi

        if [ -z "${CONFIG_PATH}" ]; then
            echo "CONFIG_PATH is required."
            exit 1
        fi

        # Using --rawfile to read config from file
        jq -n \
          --arg continuation "$CIRCLE_CONTINUATION_KEY" \
          --rawfile config "$CONFIG_PATH" \
          '{"continuation-key": $continuation, "configuration": $config}' \
          > /tmp/continuation.json

        echo "Next CircleCI config:"
        cat /tmp/continuation.json

        [[ $(curl \
                -o /dev/stderr \
                -w '%{http_code}' \
                -XPOST \
                -H "Content-Type: application/json" \
                -H "Accept: application/json"  \
                --data "@/tmp/continuation.json" \
                "https://circleci.com/api/v2/pipeline/continue") \
          -eq 200 ]]
"#,
    )
    .expect("Failed to parse continue_circleci_pipeline template");

    commands.insert("continue_circleci_pipeline".to_string(), continue_pipeline);

    // shallow_checkout command - vendored from git-shallow-clone-orb (MIT license)
    let shallow_checkout = serde_yaml::from_str(include_str!("templates/shallow_checkout.yml"))
        .expect("Failed to parse shallow_checkout template");
    commands.insert("cigen_shallow_checkout".to_string(), shallow_checkout);

    // write_submodule_commit_hash command - ensure submodule changes trigger job re-runs
    let write_submodule_commit_hash =
        serde_yaml::from_str(include_str!("templates/write_submodule_commit_hash.yml"))
            .expect("Failed to parse write_submodule_commit_hash template");
    commands.insert(
        "cigen_write_submodule_commit_hash".to_string(),
        write_submodule_commit_hash,
    );

    // cigen_calculate_sha256 command - efficient hashing with per-pattern caching
    let calculate_hash = serde_yaml::from_str(
        r#"
description: "Calculate a stable SHA-256 over file patterns with caching; exports JOB_HASH"
parameters:
  patterns:
    type: string
    description: "Newline-separated list of path patterns"
  ignorefile:
    type: string
    description: "Optional path to an ignore file to filter matches"
    default: ""
  add_ci_files:
    type: boolean
    description: "Include .circleci/* and ./scripts/ci/* in the hash"
    default: true
  unfiltered_patterns:
    type: string
    description: "Optional newline-separated patterns that bypass ignore filtering"
    default: ""
steps:
  - run:
      name: Calculate source file hash
      environment:
        PATTERNS: << parameters.patterns >>
        IGNOREFILE: << parameters.ignorefile >>
        ADD_CI: << parameters.add_ci_files >>
        UNFILTERED_PATTERNS: << parameters.unfiltered_patterns >>
      command: |
        set -euo pipefail

        if [ -z "${PATTERNS:-}" ]; then
          echo "Please provide at least one pattern" >&2
          exit 1
        fi

        if [ -z "${CI:-}" ]; then
          rm -rf /tmp/sha256-patterns
        fi

        mkdir -p /tmp/sha256-patterns
        mkdir -p /tmp/cigen

        sha() {
          if command -v sha256sum >/dev/null 2>&1; then
            sha256sum
          else
            shasum -a 256
          fi
        }

        list_files() {
          git ls-files -s --cached --modified --others --exclude-standard -- "$@" |
            grep -v '^16 ' | cut -f2- | LC_ALL=C sort -u || true
        }

        _CIGEN_IGNORE_ACTIVE=0
        _CIGEN_GITIGNORE_BACKUP=""

        restore_gitignore() {
          if [ "${_CIGEN_IGNORE_ACTIVE}" -eq 1 ]; then
            if [ -n "${_CIGEN_GITIGNORE_BACKUP}" ]; then
              mv "${_CIGEN_GITIGNORE_BACKUP}" .gitignore
            else
              rm -f .gitignore
            fi
            _CIGEN_IGNORE_ACTIVE=0
            _CIGEN_GITIGNORE_BACKUP=""
          fi
        }

        calculate_hash_for_pattern() {
          local pattern="$1"
          local cache_key
          local cache_path
          local matched
          local hash

          cache_key=$(printf '%s;%s' "$pattern" "${IGNOREFILE}" | sha | cut -d' ' -f1)
          cache_path="/tmp/sha256-patterns/${cache_key}"

          if [ -f "$cache_path" ]; then
            cat "$cache_path"
            return 0
          fi

          if [ -f "$pattern" ]; then
            matched="$pattern"
          else
            matched="$(list_files "$pattern")"
          fi

          if [ -z "$matched" ]; then
            echo "ERROR: No files matched '$pattern'" >&2
            exit 1
          fi

          if [ -n "${IGNOREFILE}" ]; then
            _CIGEN_IGNORE_ACTIVE=1
            if [ -f .gitignore ]; then
              _CIGEN_GITIGNORE_BACKUP=".gitignore.cigen-backup"
              cp .gitignore "${_CIGEN_GITIGNORE_BACKUP}"
            else
              _CIGEN_GITIGNORE_BACKUP=""
            fi

            cp "${IGNOREFILE}" .gitignore

            filtered="$(printf '%s\n' "$matched" | git check-ignore -v -n --no-index --stdin 2>/dev/null || true)"
            matched="$(printf '%s\n' "$filtered" | awk -F'\t' '/^::/ {print $2}' | LC_ALL=C sort -u)"

            restore_gitignore
            if [ -z "$matched" ]; then
              echo "ERROR: No files matched '$pattern' after filtering" >&2
              exit 1
            fi
          fi

          hash="$(printf '%s\n' "$matched" | xargs sha | cut -d' ' -f1 | sha | cut -d' ' -f1)"
          printf '%s' "$hash" > "$cache_path"
          printf '%s' "$hash"
        }

        OLD_IFS=$IFS
        IFS=$'\n'
        set -f

        combined=""
        for pat in $PATTERNS; do
          [ -z "$pat" ] && continue
          combined="${combined}$(calculate_hash_for_pattern "$pat")"
        done

        if [ -n "${UNFILTERED_PATTERNS}" ]; then
          for pat in $UNFILTERED_PATTERNS; do
            [ -z "$pat" ] && continue
            combined="${combined}$(IGNOREFILE="" calculate_hash_for_pattern "$pat")"
          done
        fi

        if [ "${ADD_CI}" = "true" ]; then
          for pat in '.circleci/*' './scripts/ci/*'; do
            combined="${combined}$(IGNOREFILE="" calculate_hash_for_pattern "$pat")"
          done
        fi

        set +f
        IFS=$OLD_IFS

        JOB_HASH=$(printf '%s' "$combined" | sha | cut -d' ' -f1)
        printf '%s' "$JOB_HASH" > /tmp/cigen/job_hash
        echo "Hash calculated: $JOB_HASH"
"#,
    )
    .expect("Failed to parse cigen_calculate_sha256 template");
    commands.insert("cigen_calculate_sha256".to_string(), calculate_hash);

    // Add more template commands here in the future
    // For example:
    // - setup_remote_docker_with_cache
    // - etc.

    commands
});

/// Check if a command name is a template command
pub fn is_template_command(command_name: &str) -> bool {
    TEMPLATE_COMMANDS.contains_key(command_name)
}

/// Get a template command definition
pub fn get_template_command(command_name: &str) -> Option<&Value> {
    TEMPLATE_COMMANDS.get(command_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_commands_loaded() {
        assert!(is_template_command("continue_circleci_pipeline"));
        assert!(is_template_command("cigen_shallow_checkout"));
        assert!(is_template_command("cigen_write_submodule_commit_hash"));
        assert!(!is_template_command("unknown_command"));
    }

    #[test]
    fn test_get_template_command() {
        let cmd = get_template_command("continue_circleci_pipeline");
        assert!(cmd.is_some());

        let cmd_value = cmd.unwrap();
        assert!(cmd_value.get("description").is_some());
        assert!(cmd_value.get("parameters").is_some());
        assert!(cmd_value.get("steps").is_some());
    }

    #[test]
    fn test_shallow_checkout_command() {
        let cmd = get_template_command("cigen_shallow_checkout");
        assert!(cmd.is_some());

        let cmd_value = cmd.unwrap();
        assert!(cmd_value.get("description").is_some());
        assert!(cmd_value.get("parameters").is_some());
        assert!(cmd_value.get("steps").is_some());

        // Check that expected parameters exist
        let params = cmd_value.get("parameters").unwrap();
        assert!(params.get("clone_options").is_some());
        assert!(params.get("fetch_options").is_some());
        assert!(params.get("keyscan_github").is_some());
        assert!(params.get("keyscan_gitlab").is_some());
        assert!(params.get("keyscan_bitbucket").is_some());
    }
}
