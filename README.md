# openshell-image-builder

[![codecov](https://codecov.io/gh/feloy/openshell-image-builder/branch/main/graph/badge.svg)](https://codecov.io/gh/feloy/openshell-image-builder)

CLI tool to build OpenShell images.

## Usage

```
openshell-image-builder [OPTIONS] <TAG>
```

| Argument / Option | Description |
| --- | --- |
| `<TAG>` | Tag for the built image (e.g. `myimage:latest`) |
| `--config <CONFIG>` | Path to config file (env: `OPENSHELL_IMAGE_BUILDER_CONFIG`) |
| `--agent <AGENT>` | Agent to install in the image (possible values: `claude`) |
| `-v` / `-vv` | Increase log verbosity (info / debug) |

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
openshell-image-builder --config /path/to/config.toml myimage:latest
```

Use an environment variable:

```sh
OPENSHELL_IMAGE_BUILDER_CONFIG=/path/to/config.toml openshell-image-builder myimage:latest
```

Enable verbose logging to trace which config file is loaded:

```sh
openshell-image-builder -v myimage:latest
```

Install the Claude agent in the image:

```sh
openshell-image-builder --agent claude myimage:latest
```

## Dev Container Features

The tool supports [Dev Container Features](https://containers.dev/implementors/features/) declared in `.kaiden/workspace.json` in the current directory.

### workspace.json schema

```json
{
  "features": {
    "<feature-ref>": {
      "<option>": "<value>"
    }
  }
}
```

Each key in `features` is a feature reference; each value is a map of options passed to the feature's `install.sh`.

### Feature references

| Format | Example | Resolves to |
| --- | --- | --- |
| OCI registry reference | `ghcr.io/devcontainers/features/rust:1` | downloaded from registry |
| Local path | `./my-feature` | `.kaiden/my-feature/` |

Local paths are resolved relative to `.kaiden/`: `./my-feature` points to `.kaiden/my-feature/`.

OCI references without an explicit registry default to `ghcr.io`. Tags and digests (`@sha256:…`) are both supported. Direct `http://` / `https://` tarball URLs are not supported.

### Installation order

Features are installed in the order defined by each feature's `installsAfter` field in its `devcontainer-feature.json`. Within the same dependency level, features are processed in alphabetical order by reference.

### Example

```json
{
  "features": {
    "ghcr.io/devcontainers/features/rust:1": {
      "version": "stable",
      "profile": "minimal"
    },
    "./my-feature": {}
  }
}
```

With the above, `./my-feature` refers to a local feature at `.kaiden/my-feature/`.

### How it works

When `.kaiden/workspace.json` is present, the tool:

1. Downloads and extracts each OCI feature into a temporary build context directory (`/tmp/openshell-image-builder…`).
2. Copies local feature directories into the same build context.
3. Passes the build context to `podman build`, where each feature is installed via:
   ```dockerfile
   COPY features/<dir>/ /tmp/feature-install/<dir>/
   RUN chmod +x /tmp/feature-install/<dir>/install.sh && \
       OPTION="value" /tmp/feature-install/<dir>/install.sh
   ```
4. Cleans up all feature files from the image with `RUN rm -rf /tmp/feature-install` after all features are installed.

Features run as root so install scripts can write to system paths.
