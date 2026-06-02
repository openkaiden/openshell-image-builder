# openshell-image-builder

[OpenShell](https://github.com/NVIDIA/OpenShell-Community) is NVIDIA's runtime environment for autonomous AI agents. It provides isolated sandboxes where agents can safely run, iterate, and be verified — without risk to the host system or other workloads.

OpenShell ships a set of [pre-built sandbox images](https://github.com/NVIDIA/OpenShell-Community), but they are general-purpose. `openshell-image-builder` lets you build your own: lightweight, workspace-specific images that contain only what you need — without writing a Containerfile by hand.

- **Base image selection** — Ubuntu, Fedora, Red Hat UBI, or Red Hat Hardened Images (HummingBird), any tag.
- **Agent installation and configuration** — pre-installed in `PATH` with scoped network access to agent-specific endpoints. Settings files can be embedded into the image from a local directory.
- **Inference configuration** — scoped network access to LLM backends.
- **Dev Container Features** — install toolchains and utilities declared in your Kaiden workspace configuration.
- **Sandbox policy** — every image ships `/etc/openshell/policy.yaml`, built from a base policy merged with inference and agent rules.

Supported agents:

- [Claude Code](https://claude.ai/code) (`--agent claude`)
- [OpenCode](https://opencode.ai) (`--agent opencode`)

Agent settings configuration includes:

- Settings files embedded from a local directory into the sandbox home.
- Automatic onboarding skip so the agent starts without interactive setup dialogs (Claude Code only, not necessary for OpenCode).

Supported inference providers:

- [Anthropic](https://www.anthropic.com) (`--inference anthropic`)
- [Vertex AI](https://cloud.google.com/vertex-ai) (`--inference vertexai`)

## Quick start

Build an image with a single command:

```sh
openshell-image-builder myimage:latest
```

`<TAG>` is the only required argument — it sets the tag for the built image. By default, the tool uses Ubuntu 24.04 as the base image.

## Configuring the base image

To use a different base image or tag, create a configuration file.

### File location

The tool looks for a `config.toml` file in this order, using the first directory found:

1. Directory given by the `--config` flag
2. Directory set in the `OPENSHELL_IMAGE_BUILDER_CONFIG` environment variable
3. The platform config directory:
   - Linux: `$XDG_CONFIG_HOME/openshell-image-builder/` (defaults to `~/.config/openshell-image-builder/`)
   - macOS: `~/Library/Application Support/openshell-image-builder/`
   - Windows: `%APPDATA%\openshell-image-builder\`

If no `config.toml` is found in the resolved directory, or the file is empty, built-in defaults are used.

If a directory is given explicitly (via `--config` or the environment variable) but it does not exist, the command fails immediately.

### Schema

```toml
[openshell_image_builder]
version = 1

[openshell_image_builder.base_image]
image = "ubuntu"   # "ubuntu", "fedora", "ubi", or "hummingbird"
tag   = "24.04"    # ubuntu: "24.04", "22.04", … — fedora: "latest", "43", "42", … — ubi: "10.2-1780377767", … — hummingbird: "latest-builder", …
```

| Field                                      | Default  | Description                  |
| ------------------------------------------ | -------- | ---------------------------- |
| `openshell_image_builder.version`          | `1`      | Configuration schema version |
| `openshell_image_builder.base_image.image` | `ubuntu` | Base image name (`ubuntu`, `fedora`, `ubi`, or `hummingbird`) |
| `openshell_image_builder.base_image.tag`   | `24.04`  | Base image tag — Ubuntu: `24.04`, `22.04`, …; Fedora: `latest`, `43`, `42`, …; UBI: `10.2-1780377767`, …; Hummingbird: `latest-builder`, … |

### Loading from a specific config directory

Pass `--config` to point to a directory explicitly (the tool reads `config.toml` inside it):

```sh
openshell-image-builder --config /path/to/config/dir myimage:latest
```

Or set the environment variable instead:

```sh
OPENSHELL_IMAGE_BUILDER_CONFIG=/path/to/config/dir openshell-image-builder myimage:latest
```

## Logging

Use `-v` (info) or `-vv` (debug) to increase log verbosity — useful for tracing which config file is loaded:

```sh
openshell-image-builder -v myimage:latest
```

## Installing an agent

Pass `--agent` to install an agent into the image.

| Agent       | Value      | Description                    |
| ----------- | ---------- | ------------------------------ |
| Claude Code | `claude`   | Anthropic's Claude Code CLI    |
| OpenCode    | `opencode` | OpenCode AI coding agent       |

```sh
openshell-image-builder --agent claude myimage:latest
openshell-image-builder --agent opencode myimage:latest
```

## Agent settings

You can pre-populate the sandbox home directory with settings files specific to an agent. Place the files under:

```
<settings dir>/agents/<agent>/
```

where `<settings dir>` is the directory described in [Configuring the base image](#configuring-the-base-image), and `<agent>` matches the value passed to `--agent` (`claude` or `opencode`).

All files and subdirectories are copied into `/sandbox/` (the sandbox user's home directory), owned by the `sandbox` user. The copy happens before the agent is installed, so the agent installer can create additional files on top without overwriting your settings.

### Example — Claude Code settings file

```sh
mkdir -p ~/.config/openshell-image-builder/agents/claude/.claude
cp ~/.claude/settings.json \
   ~/.config/openshell-image-builder/agents/claude/.claude/settings.json

openshell-image-builder --agent claude myimage:latest
```

The file will be present at `/sandbox/.claude/settings.json` in the image.

### Example — OpenCode settings file

```sh
mkdir -p ~/.config/openshell-image-builder/agents/opencode/.config/opencode
cp ~/.config/opencode/config.json \
   ~/.config/openshell-image-builder/agents/opencode/.config/opencode/config.json

openshell-image-builder --agent opencode myimage:latest
```

The file will be present at `/sandbox/.config/opencode/config.json` in the image.

### Automatic configuration — Claude Code

When `--agent claude` is used, the builder automatically creates or updates `/sandbox/.claude.json` with the following settings to skip the interactive onboarding dialogs that would otherwise appear on first launch:

- `hasCompletedOnboarding: true` — marks the setup wizard as complete.
- `projects["/sandbox"].hasTrustDialogAccepted: true` — pre-accepts the workspace trust prompt for the sandbox home directory.

If you provide your own `.claude.json` in the agent settings directory, the builder merges these fields into it, preserving any other fields you have set.

## Configuring inference

Pass `--inference` to allow the agent to reach its LLM backend. This is separate from `--agent` because the same inference provider can serve multiple agents.

| Inference | Value       | Description                         |
| --------- | ----------- | ----------------------------------- |
| Anthropic | `anthropic` | Anthropic API (`api.anthropic.com`) |
| Vertex AI | `vertexai`  | Google Vertex AI (`oauth2.googleapis.com`, `aiplatform.googleapis.com`, `*-aiplatform.googleapis.com`) |

```sh
openshell-image-builder --agent claude --inference anthropic myimage:latest
openshell-image-builder --agent opencode --inference anthropic myimage:latest
openshell-image-builder --agent claude --inference vertexai myimage:latest
openshell-image-builder --agent opencode --inference vertexai myimage:latest
```

## Sandbox policy

Every image built by this tool includes `/etc/openshell/policy.yaml`. This file is read by the OpenShell runtime and defines the sandbox security policy for the container:

- **Filesystem policy** — which paths are read-only, read-write, or inaccessible to the `sandbox` user.
- **Network policies** — which binaries are allowed to connect to which hosts and ports.

The policy is built in three layers, merged in order:

1. **Base** ([`assets/policy.yaml`](assets/policy.yaml)) — general-purpose tooling: Git operations over HTTPS and the GitHub REST API via `gh`.
2. **Inference** (added by `--inference`) — LLM backend endpoints scoped to the agent binary. For example, `--inference anthropic` adds `api.anthropic.com` and `statsig.anthropic.com`; `--inference vertexai` adds `oauth2.googleapis.com` and `aiplatform.googleapis.com` (including the `*-aiplatform.googleapis.com` wildcard).
3. **Agent** (added by `--agent`) — agent-specific endpoints. For example, `--agent claude` adds `platform.claude.com`, `raw.githubusercontent.com`, and the GitHub REST API for Claude's coding tools; `--agent opencode` adds `opencode.ai`, `registry.npmjs.org`, and `models.dev`.

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

| Format                 | Example                                  | Resolves to              |
| ---------------------- | ---------------------------------------- | ------------------------ |
| OCI registry reference | `ghcr.io/devcontainers/features/rust:1`  | downloaded from registry |
| Local path             | `./my-feature`                           | `.kaiden/my-feature/`    |

Local paths are resolved relative to `.kaiden/`: `./my-feature` points to `.kaiden/my-feature/`.

OCI references without an explicit registry default to `ghcr.io`. Tags and digests (`@sha256:…`) are both supported. Direct `http://` / `https://` tarball URLs are not supported.

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

### Installation order

Features are installed in the order defined by each feature's `installsAfter` field in its `devcontainer-feature.json`. Within the same dependency level, features are processed in alphabetical order by reference.

## Skills

Agents can be extended with *skills* — named toolkits that an agent discovers at startup. Declare skill directories in `.kaiden/workspace.json`:

```json
{
  "skills": [
    "./my-skill",
    "./.kaiden/skills/another-skill"
  ]
}
```

Each entry is a path to a directory (relative to the workspace root). The directory name becomes the skill name in the image.

During the build, each skill directory is copied into the agent's skills directory:

| Agent      | Skills directory               |
| ---------- | ------------------------------ |
| `claude`   | `/sandbox/.claude/skills/`     |
| `opencode` | `/sandbox/.opencode/skills/`   |

With `--agent claude` and `"skills": ["./my-skill"]`, the skill lands at `/sandbox/.claude/skills/my-skill/` in the image, owned by the `sandbox` user.

Skills without a corresponding `--agent` flag are silently ignored — the agent determines where skills go, so a build without an agent produces no skill COPY instructions.

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

## Full option reference

```
openshell-image-builder [OPTIONS] <TAG>
```

| Argument / Option          | Description                                                        |
| -------------------------- | ------------------------------------------------------------------ |
| `<TAG>`                    | Tag for the built image (e.g. `myimage:latest`)                    |
| `--config <CONFIG>`        | Path to config directory containing `config.toml` (env: `OPENSHELL_IMAGE_BUILDER_CONFIG`) |
| `--agent <AGENT>`          | Agent to install in the image (`claude`, `opencode`)               |
| `--inference <INFERENCE>`  | Inference server the agent will connect to (`anthropic`, `vertexai`) |
| `-v` / `-vv`               | Increase log verbosity (info / debug)                              |

## Examples

### Claude Code agent + Anthropic models provider

```
$ openshell-image-builder \
  --agent claude \
  --inference anthropic \
  sandbox_image:claude_anthropic

$ openshell provider create \
  --type generic \
  --credential ANTHROPIC_API_KEY=sk-ant-... \
  --name claude_anthropic_provider

$ openshell sandbox create \
  --from sandbox_image:claude_anthropic \
  --provider claude_anthropic_provider \
  --upload . \
  --name claude_anthropic_sandbox \
  --no-auto-providers \
  -- claude
```

### OpenCode agent + Anthropic models provider

```
$ openshell-image-builder \
  --agent opencode \
  --inference anthropic \
  sandbox_image:opencode_anthropic

$ openshell provider create \
  --type generic \
  --credential ANTHROPIC_API_KEY=sk-ant-... \
  --name opencode_anthropic_provider

$ openshell sandbox create \
  --from sandbox_image:opencode_anthropic \
  --provider opencode_anthropic_provider \
  --upload . \
  --name opencode_anthropic_sandbox \
  --no-auto-providers \
  -- opencode
```

### Claude Code agent + Vertex AI models provider

```
$ openshell-image-builder \
  --agent claude \
  --inference vertexai \
  sandbox_image:claude_vertexai

# change value of ANTHROPIC_VERTEX_PROJECT_ID and CLOUD_ML_REGION
$ openshell sandbox create \
  --from sandbox_image:claude_vertexai \
  --upload . \
  --name claude_vertexai_sandbox \
  --no-auto-providers \
  --no-tty \
  -- bash -c '(\
    echo export CLAUDE_CODE_USE_VERTEX=1; \
    echo export ANTHROPIC_VERTEX_PROJECT_ID=my-gcp-project; \
    echo export CLOUD_ML_REGION=global \
  ) >> /sandbox/.bashrc'

$ openshell sandbox upload \
  claude_vertexai_sandbox \
  $HOME/.config/gcloud/application_default_credentials.json \
  /sandbox/.config/gcloud/application_default_credentials.json

$ openshell sandbox connect claude_vertexai_sandbox

sandbox:~$ claude
```

### OpenCode agent + Vertex AI models provider

```
$ openshell-image-builder \
  --agent opencode \
  --inference vertexai \
  sandbox_image:opencode_vertexai

# change value of GOOGLE_CLOUD_PROJECT and VERTEX_LOCATION
$ openshell sandbox create \
  --from sandbox_image:opencode_vertexai \
  --upload . \
  --name opencode_vertexai_sandbox \
  --no-auto-providers \
  --no-tty \
  -- bash -c '(\
    echo export GOOGLE_CLOUD_PROJECT=my-gcp-project; \
    echo export VERTEX_LOCATION=global; \
    echo export GOOGLE_APPLICATION_CREDENTIALS=/sandbox/.config/gcloud/application_default_credentials.json \
  ) >> /sandbox/.bashrc'

$ openshell sandbox upload \
  opencode_vertexai_sandbox \
  $HOME/.config/gcloud/application_default_credentials.json \
  /sandbox/.config/gcloud/application_default_credentials.json

$ openshell sandbox connect opencode_vertexai_sandbox

sandbox:~$ opencode

# select a model with /models
```
