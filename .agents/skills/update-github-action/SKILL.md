---
name: update-github-action
description: Add or update a GitHub Actions step — fetch the latest release tag and its commit SHA, write the pinned uses line with a version comment
argument-hint: "<owner/repo> [tag]"
---

# Update GitHub Action

Pin a GitHub Actions step to an exact commit SHA with a version comment.

## Description

All `uses:` lines in `.github/workflows/` must be pinned to a commit SHA, not a tag. Tags are mutable — a maintainer can silently move them to a different commit. A SHA is immutable.

The required format is:

```yaml
uses: actions/checkout@df4cb1c069e1874edd31b4311f1884172cec0e10 # v6.0.3
```

Dependabot is configured (`.github/dependabot.yml`) to open daily PRs that keep these SHAs current. When you add a new action or need to update one manually — e.g. because you are writing a new workflow step — follow the steps below.

## Step 1 — find the latest release tag

```bash
gh api repos/<owner>/<repo>/releases/latest --jq '.tag_name'
```

Example:

```bash
gh api repos/actions/cache/releases/latest --jq '.tag_name'
# → v5.0.5
```

If the action has no GitHub releases (e.g. `dtolnay/rust-toolchain`), use `git/refs/tags` to list tags instead:

```bash
gh api repos/dtolnay/rust-toolchain/git/refs/tags --jq '.[-1].ref'
# → refs/tags/master  (this action uses a different convention — see below)
```

## Step 2 — resolve the commit SHA

```bash
gh api repos/<owner>/<repo>/git/ref/tags/<tag> --jq '.object.sha'
```

Example:

```bash
gh api repos/actions/cache/git/ref/tags/v5.0.5 --jq '.object.sha'
# → 27d5ce7f107fe9357f9df03efb73ab90386fccae
```

**Annotated tag check**: if the returned SHA is the SHA of a tag object rather than a commit (both are 40-hex strings; you cannot tell from length alone), dereference it:

```bash
TYPE=$(gh api repos/<owner>/<repo>/git/ref/tags/<tag> --jq '.object.type')
# if TYPE == "tag", dereference:
gh api repos/<owner>/<repo>/git/tags/<sha-from-step-above> --jq '.object.sha'
```

Most release tags are lightweight and the first SHA is the commit SHA directly. When in doubt, paste the SHA into `https://github.com/<owner>/<repo>/commit/<sha>` — if the page loads, it is a commit SHA.

## Step 3 — write the `uses:` line

Combine the SHA and the tag into a single line:

```yaml
uses: actions/cache@27d5ce7f107fe9357f9df03efb73ab90386fccae # v5.0.5
```

Rules:
- No space between `@` and the SHA.
- The comment is `# <tag>` exactly as returned by Step 1 — do not normalise (keep the `v` prefix).
- Update **every** occurrence of the same action across all workflow files.

## Special case — `dtolnay/rust-toolchain`

This action does not follow standard GitHub releases and does not have a per-version tag. The convention used in this repo is to pin it to whatever SHA `master` currently resolves to and comment `# v1 stable`:

```bash
gh api repos/dtolnay/rust-toolchain/git/ref/heads/master --jq '.object.sha'
```

```yaml
uses: dtolnay/rust-toolchain@<sha> # v1 stable
```

Always add `toolchain: stable` under `with:` — this action has no default and will error without it:

```yaml
- uses: dtolnay/rust-toolchain@e97e2d8cc328f1b50210efc529dca0028893a2d9 # v1 stable
  with:
    toolchain: stable
    components: rustfmt, clippy   # add only the components you need
```

## Current pinned actions (reference)

| Action | Tag | Workflows |
|---|---|---|
| `actions/checkout` | `v6.0.3` | all three |
| `dtolnay/rust-toolchain` | `v1 stable` | `pr-check.yml`, `integration.yml`, `release.yml` |
| `actions/cache` | `v5.0.5` | `pr-check.yml`, `integration.yml` |
| `taiki-e/install-action` | `v2.81.4` | `pr-check.yml` |
| `codecov/codecov-action` | `v6.0.1` | `pr-check.yml` |
| `actions/upload-artifact` | `v7.0.1` | `release.yml` |
| `actions/download-artifact` | `v8.0.1` | `release.yml` |

When adding a new step that reuses one of these, copy the SHA from the table above rather than re-fetching (Dependabot keeps them current). Fetch a fresh SHA only when adding an action not already in the table.

## Checklist

- [ ] Latest release tag fetched with `gh api .../releases/latest`
- [ ] Commit SHA fetched with `gh api .../git/ref/tags/<tag>`
- [ ] Annotated tag dereferenced if needed (confirmed SHA loads as a commit on GitHub)
- [ ] `uses:` line written as `owner/repo@<sha> # <tag>`
- [ ] All occurrences across all workflow files updated to the same SHA
- [ ] `dtolnay/rust-toolchain` has `toolchain: stable` under `with:`
