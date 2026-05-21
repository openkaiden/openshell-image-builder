# openshell-image-builder

CLI tool to build OpenShell images.

## Configuration

### File location

The tool looks for a configuration file in this order, using the first one found:

1. Path given by the `--config` flag
2. Path set in the `OPENSHELL_IMAGE_BUILDER_CONFIG` environment variable
3. The platform config directory:
   - Linux: `$XDG_CONFIG_HOME/openshell-image-builder/config.toml` (defaults to `~/.config/openshell-image-builder/config.toml`)
   - macOS: `~/Library/Application Support/openshell-image-builder/config.toml`
   - Windows: `%APPDATA%\openshell-image-builder\config.toml`

If no file is found, or the file is empty, built-in defaults are used.

If a path is given explicitly (via `--config` or the environment variable) but the file does not exist, the command fails immediately.

### Schema

```toml
[openshell_image_builder]
version = 1

[openshell_image_builder.base_image]
image = "fedora"   # or "ubuntu"
tag   = "latest"   # or "43", "24.04", etc.
```

| Field                                      | Default  | Description                  |
| ------------------------------------------ | -------- | ---------------------------- |
| `openshell_image_builder.version`          | `1`      | Configuration schema version |
| `openshell_image_builder.base_image.image` | `fedora` | Base image name              |
| `openshell_image_builder.base_image.tag`   | `latest` | Base image tag               |

### Examples

Use a specific config file:

```sh
openshell-image-builder --config /path/to/config.toml
```

Use an environment variable:

```sh
OPENSHELL_IMAGE_BUILDER_CONFIG=/path/to/config.toml openshell-image-builder
```

Enable verbose logging to trace which config file is loaded:

```sh
openshell-image-builder -v
```
