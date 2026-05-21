# openshell-image-builder

Rust CLI using clap (derive API) for argument parsing.

## Versioning

The version in `Cargo.toml` follows `x.y.z-next` on the `main` branch (e.g. `0.1.0-next`). Never remove the `-next` suffix manually — CI handles that on release.

## Before committing

Always run the full check suite locally before staging:

```bash
cargo fmt --check && cargo clippy -- -D warnings && cargo test
```

## Rust gotchas

- Imports only used in tests must live inside the `#[cfg(test)]` module, not at the top level — otherwise clippy's `-D warnings` flags them as unused.
- `dtolnay/rust-toolchain` requires an explicit `toolchain: stable` input, it has no default.

## GitHub Actions

Pin actions by commit SHA with a version comment, using the actual latest release — not just the latest major tag:

```yaml
uses: actions/checkout@<sha> # v6.0.2
```

Before writing a `uses:` line, fetch the latest release and its SHA:

```bash
gh api repos/<owner>/<repo>/releases/latest --jq '.tag_name'
gh api repos/<owner>/<repo>/git/ref/tags/<tag> --jq '.object.sha'
```

## User context

User is new to Rust — explain Rust-specific concepts (macros, import scoping, cargo commands) from first principles.
