# Templating System

Cigen uses [Tera](https://tera.netlify.app/) for templating, providing flexible variable substitution and control structures in configuration files.

## File Types

### Standard YAML Files (`.yml`, `.yaml`)

- Support basic Tera templating while maintaining valid YAML structure
- Can use variable substitution: `image: postgres:{{ postgres_version }}`
- Can use inline loops within quotes: `command: "{% for env in envs %}{{ env }} {% endfor %}"`
- Must remain valid YAML for IDE schema validation and helpful error reporting
- Recommended for most configuration files

### Template Files (`.yml.tera`, `.yaml.tera`)

- Full Tera templating power with control structures
- Can break YAML syntax with conditionals and loops
- No schema validation attempted by IDEs
- Use when you need complex templating logic

## Variable Sources

Variables are resolved in the following order (later sources override earlier ones):

1. **Vars file** (e.g. `config/vars.yml` or inline `vars:` section in config)
2. **Environment variables** (prefixed with `CIGEN_VAR_`)
3. **Command line** (`--var key=value`)

### Vars File

```yaml
vars:
  postgres_version: 16.1
  redis_version: 7.4.0
  minio_version: RELEASE.2025-06-26T18-44-10Z
```

### Environment Variables

```bash
export CIGEN_VAR_POSTGRES_VERSION=16.2
export CIGEN_VAR_REDIS_VERSION=7.5.0
```

### Command Line

```bash
cigen generate --var postgres_version=16.3 --var redis_version=7.6.0
```

## Examples

### Basic Variable Substitution (.yml)

```yaml
# config.yml
jobs:
  setup:
    docker:
      - image: cimg/postgres:{{ postgres_version }}
    steps:
      - run: echo "Using PostgreSQL {{ postgres_version }}"
```

### Inline Loops (.yml)

```yaml
# Must be within quotes to maintain valid YAML
environment:
  PATH: '{% for dir in path_dirs %}{{ dir }}:{% endfor %}$PATH'
```

### Complex Templating (.yml.tera)

```yaml
# workflows.yml.tera
{% if use_postgres %}
services:
  postgres:
    image: postgres:{{ postgres_version }}
    environment:
      POSTGRES_PASSWORD: {{ db_password }}
{% endif %}

{% for env in environments %}
deploy_{{ env }}:
  docker:
    - image: myapp:{{ version }}
  steps:
    - run: deploy to {{ env }}
{% endfor %}
```

## Built-in Functions

### `read(filename)`

Reads the contents of a file relative to the config directory:

```yaml
steps:
  - run: |
      {{ read('scripts/setup.sh') | trim }}
```

### Filters

All standard Tera filters are available:

- `trim` - Remove whitespace
- `upper` - Convert to uppercase
- `lower` - Convert to lowercase
- `replace` - Replace text
- And many more...

## Best Practices

1. **Use `.yml` for most files** - Start with standard YAML and only use `.yml.tera` when you need complex control structures
2. **Keep vars organized** - Use a dedicated `vars.yml` file for complex variable sets
3. **Environment-specific overrides** - Use environment variables for deployment-specific values
4. **Readable templates** - Use meaningful variable names and add comments for complex logic
5. **Test templates** - Always test template rendering with different variable combinations

## Error Handling

- **Template errors** show the exact line and column where the error occurred
- **Undefined variables** will cause Tera to crash with an error - all variables used in templates must be defined
- **Variable errors** indicate which variables are missing or invalid
- **YAML errors** (for `.yml` files) show both the template source and rendered output location
