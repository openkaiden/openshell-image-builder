---
name: coderabbit-review
description: Fetch all CodeRabbitAI review comments on the current PR, triage them by severity, and address the actionable ones
argument-hint: "[pr-number]"
---

# CodeRabbit Review

Fetch and act on CodeRabbitAI review feedback for a pull request.

## Instructions

### Step 1 — identify the PR and its base repository

If a PR number was given as argument, use it directly and skip to Step 2 — the base repo is the `upstream` remote (see below).

Otherwise, resolve the PR from the current branch. In a fork workflow the PR lives on the upstream repo, not the fork, so `gh pr view` without `--repo` will fail. Resolve the upstream slug first:

```bash
git remote get-url upstream
# https://github.com/owner/repo.git  →  owner/repo
# git@github.com:owner/repo.git      →  owner/repo
```

Strip the protocol prefix and `.git` suffix to get `<owner/repo>`, then look up the PR:

```bash
gh pr view --repo <owner/repo> --json number,url
```

Use `number` as the PR number and `<owner/repo>` as the base repo slug for all subsequent API calls.

### Step 2 — fetch inline review comments

These are the most actionable: each one points to a specific file and line.

```bash
gh api repos/<owner/repo>/pulls/<pr-number>/comments \
  --jq '[.[] | select(.user.login == "coderabbitai[bot]") | {path, line, body}]'
```

### Step 3 — fetch the review summary

The formal review body contains the "Actionable comments posted: N" summary and nitpick rollup.

```bash
gh api repos/<owner/repo>/pulls/<pr-number>/reviews \
  --jq '[.[] | select(.user.login == "coderabbitai[bot]") | {state, body}]'
```

### Step 4 — triage the comments

CodeRabbitAI labels each inline comment with severity markers in the body text. Process them in this order:

| Marker | Action |
|---|---|
| `⚠️ Potential issue` / `🟠 Major` | Fix first — these are bugs or correctness problems |
| `🛠️ Refactor suggestion` / `🟠 Major` | Fix if straightforward — structural improvements |
| `⚠️ Potential issue` / `🟡 Minor` | Fix if quick — correctness with low risk |
| `🛠️ Refactor suggestion` / `🟡 Minor` | Fix at discretion |
| `🧹 Nitpick` | Fix only if trivial; skip otherwise |
| `♻️ Duplicate comments` | Skip — already covered by another inline comment |

For each comment you decide to address: read the flagged file at the indicated `path` and `line`, understand the concern, then apply the fix.

### Step 5 — address the comments

For each actionable comment:

1. Read the file at `path` around `line` (use the Read tool).
2. Understand what CodeRabbitAI is pointing out — do not blindly apply the suggested fix if it conflicts with the existing code style or project conventions.
3. Apply the fix with the Edit tool.
4. Run `/check` to confirm the suite still passes.

### Step 6 — report

Summarise what was fixed, what was intentionally skipped (and why), and whether any comment requires a design decision from the author.
