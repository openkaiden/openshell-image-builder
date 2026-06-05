---
name: check
description: Run the pre-commit check suite — cargo fmt, clippy, and unit tests — and fix formatting automatically if needed
---

# Check

Run the mandatory pre-commit check suite for openshell-image-builder.

## Description

CLAUDE.md requires this suite to pass before staging any commit:

```bash
cargo fmt --check && cargo clippy -- -D warnings && cargo test
```

This skill runs the three steps in order, stopping at the first failure. If `cargo fmt --check` fails it automatically applies `cargo fmt` and reports which files were reformatted before continuing.

## Instructions

Run the steps below in sequence. Stop and report the failure if any step exits non-zero (after the fmt auto-fix described in step 1).

### Step 1 — formatting

```bash
cargo fmt --check
```

If this fails (exit non-zero), run:

```bash
cargo fmt
```

Then tell the user which files were reformatted. Continue to step 2.

### Step 2 — lints

```bash
cargo clippy -- -D warnings
```

`-D warnings` promotes every clippy warning to a hard error. Common causes:

- Unused imports at the top level that belong inside `#[cfg(test)]` — move them into the test module.
- Dead code, redundant clones, needless borrows — fix as indicated by the diagnostic.

If this step fails, report the clippy output and stop.

### Step 3 — unit tests

```bash
cargo test
```

This runs only the unit tests embedded in `src/`. Integration tests in `tests/` are excluded here (they require podman and are opt-in via `/integration-tests`).

If this step fails, report the failing test names and their output, then stop.

### Final report

If all three steps pass (or step 1 was auto-fixed and steps 2–3 pass), report:

- Whether formatting was auto-fixed (list files) or was already clean.
- Clippy: clean.
- Tests: N passed.
- Whether the tree is ready to commit or needs a `git add` for the reformatted files.
