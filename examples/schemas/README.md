# Sldr JSON Schemas & Example Configs

This directory contains JSON schemas and example configurations for sldr. These files enable:

- **IDE Autocompletion**: Get suggestions when editing config files with editors like VS Code
- **Validation**: Catch configuration errors before running sldr
- **Documentation**: Inline documentation for all configuration options

## Available Schemas

| Schema | Purpose | Reference in Config |
|--------|---------|-------------------|
| `sldr.config.schema.json` | Main sldr configuration (`config.toml`) | ✓ |
| `sldr.flavor.schema.json` | Flavor/theme configuration (`flavor.toml`) | ✓ |
| `sldr.skeleton.schema.json` | Presentation skeleton configuration (`skeleton.toml`) | ✓ |

## Example Configs

Located in parent `examples/` directory:

| File | Purpose |
|------|---------|
| `config.toml` | Main configuration with all options documented |
| `example-flavor.toml` | Flavor configuration showing all visual settings |
| `example-skeleton.toml` | Skeleton configuration showing presentation structure |

## Using Schemas

### In VS Code

1. Install **Even Better TOML** extension
2. Open any `.toml` config file
3. Start typing to see autocompletion and validation

### In Other Editors

Most modern editors support JSON Schema validation for TOML files. Configure your editor to use schema from `$schema` field in your config files.

## Schema Reference

All sldr config files include a `$schema` field pointing to canonical schema location:

```toml
"$schema" = "https://raw.githubusercontent.com/byteowlz/schemas/refs/heads/main/sldr/sldr.config.schema.json"
```

## Generating Schemas & Examples

Schemas and example configs are automatically generated from Rust source code using `schemars` crate:

```bash
just schemas
```

This generates:
- JSON schemas in `examples/schemas/`
- Example configs in `examples/` with inline comments

## Updating Schemas

After modifying config structures in `sldr-core`, regenerate schemas and examples:

```bash
just schemas
just copy-schemas
```

## Schema Repository

The canonical schemas are published to `byteowlz/schemas` repository. Use copy script to push updates:

```bash
./scripts/copy_config_schema.sh
```
