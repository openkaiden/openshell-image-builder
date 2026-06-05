---
name: add-base-image
description: Step-by-step checklist for adding a new base image to openshell-image-builder, covering the Containerfile match arm, unit tests, and integration tests
argument-hint: "<image-name> <registry-url> <package-manager>"
---

# Add Base Image

End-to-end checklist for supporting a new base image in openshell-image-builder.

## Description

The base image is selected by the user via `config.toml`:

```toml
[openshell_image_builder.base_image]
image = "myimage"
tag   = "latest"
```

The name is a plain string — no enum, no CLI flag change. Adding a new base image only touches two files: `src/containerfile.rs` and `tests/integration_test.rs`.

## Step 1 — decide the package manager

The codebase already has two helpers in `src/containerfile.rs`:

- **`ubuntu_system_stage(tag)`** — APT, `docker.io/library/ubuntu`.
- **`dnf_system_stage(base_image, tag, packages)`** — DNF, used by fedora, ubi, and hummingbird.

For a new image, reuse the helper whose package manager matches. If the new image uses a third package manager (e.g., apk for Alpine), write a new `fn alpine_system_stage(tag: &str) -> String` following the same pattern as the existing two: `FROM <registry>:{tag} AS system`, install packages, create `supervisor` and `sandbox` users.

Required users and groups — every base image must create them the same way:

```sh
groupadd -r supervisor && useradd -r -g supervisor -s /usr/sbin/nologin supervisor
groupadd -r sandbox   && useradd -r -g sandbox -d /sandbox -s /bin/bash sandbox
```

Minimum packages needed regardless of package manager: `ca-certificates`, `curl`, `openssh-server` (or equivalent sshd), `tar`, `which`, `procps` (or equivalent).

## Step 2 — update `README.md`

Three places reference the supported base image names; all must be kept in sync:

1. **Introduction bullet** (line ~9) — the prose list "Ubuntu, Fedora, Red Hat UBI, or Red Hat Hardened Images (HummingBird)". Add the new image name in the same style.

2. **"Base images" subsection** — the four named `config.toml` example blocks. Add a new block with the image name as a heading and the typical default tag:

   ```markdown
   **My Image**

   \`\`\`toml
   [openshell_image_builder.base_image]
   image = "myimage"
   tag   = "latest"
   \`\`\`
   ```

3. **"Full schema reference" table** — two occurrences:
   - The inline comment on the `image` key: `# "ubuntu", "fedora", "ubi", or "hummingbird"` → add `"myimage"`.
   - The `base_image.image` row description and the `base_image.tag` row description — add the new name and its typical tag examples.

## Step 3 — add the match arm in `generate()` (`src/containerfile.rs`)

Inside `generate()`, add a new arm to the `match config.base_image.image.as_str()` block before the catch-all `image =>` arm:

```rust
"myimage" => dnf_system_stage(
    "registry.example.com/myimage",
    tag,
    &[
        "ca-certificates",
        "curl",
        "openssh-server",
        "procps-ng",
        "tar",
        "which",
        // add further packages the image needs
    ],
),
```

The `final_stage` is shared across all base images and never needs changes.

## Step 4 — unit tests (`src/containerfile.rs`, `#[cfg(test)]`)

Add a config helper alongside the existing ones (`fedora_config`, `ubi_config`, etc.):

```rust
fn myimage_config() -> Config {
    Config {
        version: 1,
        base_image: BaseImageConfig {
            image: "myimage".to_string(),
            tag: "latest".to_string(),
        },
    }
}
```

Then add these tests — every existing base image has all of them, keep the set complete:

```rust
#[test]
fn myimage_generates_successfully() {
    assert!(build_cf(&myimage_config(), None, &[], false, &[]).is_ok());
}

#[test]
fn myimage_containerfile_contains_tag() {
    let content = build_cf(&myimage_config(), None, &[], false, &[]).unwrap();
    assert!(content.contains("FROM registry.example.com/myimage:latest AS system"));
}

#[test]
fn myimage_containerfile_tag_is_substituted() {
    let content = build_cf(&myimage_config(), None, &[], false, &[]).unwrap();
    assert!(!content.contains("{tag}"));
}

#[test]
fn myimage_with_agent_includes_install() {
    let content = build_cf(&myimage_config(), Some(&MockAgent), &[], false, &[]).unwrap();
    assert!(content.contains("RUN echo mock-agent"));
}

#[test]
fn myimage_agent_install_runs_as_sandbox_user() {
    let content = build_cf(&myimage_config(), Some(&MockAgent), &[], false, &[]).unwrap();
    let user_pos    = content.find("USER sandbox").unwrap();
    let install_pos = content.find("RUN echo mock-agent").unwrap();
    assert!(install_pos > user_pos, "agent install must appear after USER sandbox");
}

#[test]
fn myimage_without_agent_omits_install() {
    let content = build_cf(&myimage_config(), None, &[], false, &[]).unwrap();
    assert!(!content.contains("RUN echo mock-agent"));
}

#[test]
fn myimage_copies_policy_yaml() {
    let content = build_cf(&myimage_config(), None, &[], false, &[]).unwrap();
    assert!(content.contains("COPY policy.yaml /etc/openshell/policy.yaml"));
}
```

Also extend the `home_env_set_to_sandbox` test to include the new config, so it keeps covering every base image:

```rust
fn home_env_set_to_sandbox() {
    for content in [
        ...,
        build_cf(&myimage_config(), None, &[], false, &[]).unwrap(),
    ] {
        assert!(content.contains("ENV HOME=/sandbox"));
    }
}
```

## Step 5 — integration tests (`tests/integration_test.rs`)

### Config helper

If the new image needs a non-default tag, add a config dir helper alongside `fedora_config_dir`:

```rust
fn myimage_config_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("config.toml"),
        "[openshell_image_builder.base_image]\nimage = \"myimage\"\ntag = \"latest\"\n",
    )
    .unwrap();
    dir
}
```

### Image singletons and accessors

Add one `OnceLock` and one accessor per agent × inference combination. Follow the naming convention exactly — the integration test matrix currently covers these combinations for every base image:

| Combination           | Accessor name                  | Extra args                                      |
|-----------------------|--------------------------------|-------------------------------------------------|
| (no agent)            | `myimage_image`                | `[]`                                            |
| claude + anthropic    | `myimage_claude_image`         | `["--agent", "claude", "--inference", "anthropic"]` |
| opencode + anthropic  | `myimage_opencode_image`       | `["--agent", "opencode", "--inference", "anthropic"]` |
| claude + vertexai     | `myimage_claude_vertexai_image`| `["--agent", "claude", "--inference", "vertexai"]` |
| opencode + vertexai   | `myimage_opencode_vertexai_image`| `["--agent", "opencode", "--inference", "vertexai"]` |
| opencode + ollama     | `myimage_opencode_ollama_image`| `["--agent", "opencode", "--inference", "ollama"]` |

Example for the no-agent variant (pass the `--config` flag when a config dir is needed):

```rust
static MYIMAGE_IMAGE: OnceLock<String> = OnceLock::new();

fn myimage_image() -> &'static str {
    MYIMAGE_IMAGE.get_or_init(|| {
        let config = myimage_config_dir();
        build_image(
            "openshell-test-myimage:integration",
            &["--config", config.path().to_str().unwrap()],
        )
    })
}
```

### `image_tests!` calls

Add one call per combination in the matrix block (around line 678):

```rust
image_tests!(myimage,                  myimage_image,                  has_claude: false, has_opencode: false, has_anthropic: false, has_vertexai: false, has_ollama: false);
image_tests!(myimage_claude,           myimage_claude_image,           has_claude: true,  has_opencode: false, has_anthropic: true,  has_vertexai: false, has_ollama: false);
image_tests!(myimage_opencode,         myimage_opencode_image,         has_claude: false, has_opencode: true,  has_anthropic: true,  has_vertexai: false, has_ollama: false);
image_tests!(myimage_claude_vertexai,  myimage_claude_vertexai_image,  has_claude: true,  has_opencode: false, has_anthropic: false, has_vertexai: true,  has_ollama: false);
image_tests!(myimage_opencode_vertexai,myimage_opencode_vertexai_image,has_claude: false, has_opencode: true,  has_anthropic: false, has_vertexai: true,  has_ollama: false);
image_tests!(myimage_opencode_ollama,  myimage_opencode_ollama_image,  has_claude: false, has_opencode: true,  has_anthropic: false, has_vertexai: false, has_ollama: true);
```

### Cleanup

Add all six tags to the `cleanup_images` array in the `#[ctor::dtor]` at the bottom of the file:

```rust
"openshell-test-myimage:integration",
"openshell-test-myimage-claude:integration",
"openshell-test-myimage-opencode:integration",
"openshell-test-myimage-claude-vertexai:integration",
"openshell-test-myimage-opencode-vertexai:integration",
"openshell-test-myimage-opencode-ollama:integration",
```

## Checklist

- [ ] README intro bullet updated with the new image name
- [ ] README "Base images" section: new named `config.toml` block added
- [ ] README schema reference: inline comment and table rows updated
- [ ] Package manager identified; new stage helper written if needed
- [ ] Match arm added in `generate()` before the catch-all `image =>` arm
- [ ] `myimage_config()` helper added in `src/containerfile.rs` tests
- [ ] All seven unit tests added (generates, tag present, tag substituted, with/without agent, agent order, policy yaml)
- [ ] `home_env_set_to_sandbox` extended with the new config
- [ ] `myimage_config_dir()` helper added to integration tests if the image uses a non-default tag
- [ ] Six `OnceLock` statics and six accessor functions added
- [ ] Six `image_tests!` calls added in the matrix block
- [ ] Six tags added to `cleanup_images`
- [ ] `/check` passes (fmt + clippy + unit tests)
- [ ] `/copyright-headers` run if a new `.rs` file was created
