# Contributing to openshell-image-builder

## Prerequisites

| Tool | Version | Purpose |
|---|---|---|
| Rust | stable | build and test |
| podman | any recent | integration tests |
| gh | any recent | GitHub workflows, action SHA lookup |

Install Rust via [rustup](https://rustup.rs/):

```sh
rustup toolchain install stable
```

## Getting started

```sh
git clone https://github.com/openkaiden/openshell-image-builder.git
cd openshell-image-builder
cargo build
cargo run -- --help
```

## Source layout

```
src/
├── main.rs              — CLI (clap derive), build orchestration, build_policy()
├── agent/
│   ├── mod.rs           — Agent trait, AgentKind enum, from_kind()
│   ├── claude.rs        — ClaudeAgent implementation
│   └── opencode/        — OpencodeAgent + per-inference config submodules
├── inference/
│   ├── mod.rs           — Inference trait, InferenceKind enum, from_kind()
│   ├── anthropic.rs     — AnthropicInference
│   ├── ollama.rs        — OllamaInference
│   └── vertexai.rs      — VertexAiInference
├── config.rs            — config.toml loading and defaults
├── containerfile.rs     — Containerfile generation (generate(), stage helpers)
├── build.rs             — Runner trait, PodmanRunner, build()
└── policy/
    └── mod.rs           — Serde types for policy YAML, parse/serialize helpers
assets/
└── policy.yaml          — base sandbox policy (filesystem, process, network)
tests/
└── integration_test.rs  — full image-build integration tests (require podman)
```

## Development workflow

### Pre-commit check suite

Run this before every commit. CI runs the same three commands and fails the PR if any of them fail:

```sh
cargo fmt --check && cargo clippy -- -D warnings && cargo test
```

If `cargo fmt --check` fails, fix formatting with:

```sh
cargo fmt
```

### Integration tests

Integration tests build real container images with podman. They are marked `#[ignore]` and skipped by the default `cargo test` run. Run them explicitly:

```sh
cargo test --test integration_test -- --include-ignored --test-threads=1
```

`--test-threads=1` is required: each image is built at most once per process via `OnceLock`, and parallel threads would race the singletons.

The tests tag images as `openshell-test-*:integration`. They are cleaned up automatically by a `#[ctor::dtor]` destructor at the end of the test run.

## License headers

Every `.rs` file must start with an Apache 2.0 license header. Copy from any existing source file. The required format is:

```rust
// Copyright (C) 2026 Red Hat, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// SPDX-License-Identifier: Apache-2.0
```

## Code conventions

**Test-only imports** belong inside `#[cfg(test)]`, not at the top level. Clippy's `-D warnings` flags top-level imports that are only used in tests as unused:

```rust
// wrong — clippy error outside test builds
use some_crate::TestHelper;

// correct
#[cfg(test)]
mod tests {
    use super::*;
    use some_crate::TestHelper;
}
```

**No clippy warnings**: the CI runs `cargo clippy -- -D warnings` and treats every warning as an error.

**Sandbox policy YAML**: the `src/policy/mod.rs` structs use `#[serde(deny_unknown_fields)]`. Adding an unrecognised field to `assets/policy.yaml` or any `policy_yaml()` fragment will cause a parse error at build time.

## Adding new features

The `.agents/skills/` directory contains step-by-step checklists for the most common types of changes. These cover every file that needs to change and the exact code patterns to follow — useful as a reference even if you are not using an AI agent:

| Change type | Skill | Guide |
|---|---|---|
| New supported base OS image | `/add-base-image` | [`.agents/skills/add-base-image/SKILL.md`](.agents/skills/add-base-image/SKILL.md) |
| New CLI flag | `/add-cli-flag` | [`.agents/skills/add-cli-flag/SKILL.md`](.agents/skills/add-cli-flag/SKILL.md) |
| New inference provider | `/add-inference` | [`.agents/skills/add-inference/SKILL.md`](.agents/skills/add-inference/SKILL.md) |
| New agent | `/add-agent` | [`.agents/skills/add-agent/SKILL.md`](.agents/skills/add-agent/SKILL.md) |
| Sandbox network policy changes | `/sandbox-policy` | [`.agents/skills/sandbox-policy/SKILL.md`](.agents/skills/sandbox-policy/SKILL.md) |

## Versioning

`Cargo.toml` always carries a `-next` suffix on the `main` branch (e.g. `0.10.0-next`). Do not remove or change it manually — the release workflow strips the suffix and bumps to the next minor version automatically when a release tag is pushed.

## GitHub Actions

All `uses:` lines must be pinned to a commit SHA, not a tag:

```yaml
uses: actions/checkout@df4cb1c069e1874edd31b4311f1884172cec0e10 # v6.0.3
```

Before adding or updating an action, fetch the latest release tag and its commit SHA:

```sh
gh api repos/<owner>/<repo>/releases/latest --jq '.tag_name'
gh api repos/<owner>/<repo>/git/ref/tags/<tag> --jq '.object.sha'
```

Dependabot opens daily PRs to keep existing SHAs current — you only need to fetch a fresh SHA when adding an action that is not already used in the workflows. See [`.agents/skills/update-github-action/SKILL.md`](.agents/skills/update-github-action/SKILL.md) for the full procedure.

## Submitting a pull request

1. Fork the repository and create a branch from `main`.
2. Make your changes. Run the check suite: `cargo fmt --check && cargo clippy -- -D warnings && cargo test`.
3. Add the Apache 2.0 header to any new `.rs` files.
4. Open a PR against `main` in `openkaiden/openshell-image-builder`.

CI runs automatically on every PR:
- **PR Check** (`pr-check.yml`) — fmt, clippy, unit tests on Ubuntu, macOS, and Windows; coverage upload to Codecov.
- **Integration Tests** (`integration.yml`) — full image builds with podman on Ubuntu.

Both must pass before a PR can be merged.
