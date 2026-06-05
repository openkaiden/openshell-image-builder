---
name: work-on-issue
description: Fetch an issue from the upstream repository, understand its scope, map it to existing skills, and produce an implementation plan
argument-hint: "<issue-id>"
---

# Work on Issue

Fetch an upstream issue and turn it into a concrete implementation plan.

## Instructions

### Step 1 — resolve the upstream repository

```bash
git remote get-url upstream
```

Extract the `owner/repo` slug from the URL. Both HTTPS and SSH forms are valid:

- `https://github.com/owner/repo.git` → `owner/repo`
- `git@github.com:owner/repo.git` → `owner/repo`

### Step 2 — fetch the issue

```bash
gh issue view <issue-id> --repo <owner/repo> --comments
```

Read the full output: title, body, labels, and every comment. If the issue references other issues or PRs, fetch those too:

```bash
gh issue view <other-id> --repo <owner/repo> --comments
gh pr view <pr-id> --repo <owner/repo> --comments
```

### Step 3 — understand the scope

Identify what kind of change the issue is asking for, then check whether an existing skill already covers this type of work:

| If the issue asks for… | Use skill |
|---|---|
| A new supported base OS image | `/add-base-image` |
| A new CLI flag or option | `/add-cli-flag` |
| A new inference provider | `/add-inference` |
| A new agent | `/add-agent` |
| A new sandbox network rule | `/sandbox-policy` |

If an existing skill applies, load it — it lists every file that needs to change and the exact patterns to follow.

### Step 4 — explore the relevant code

Read the source files most likely affected by the issue. Use grep, find, and Read to locate the relevant functions, structs, and tests. Do not skip this step — the plan must name specific files and line ranges, not describe changes in the abstract.

### Step 5 — enter plan mode

Call `EnterPlanMode` and write a concrete implementation plan that covers:

- Every file that needs to change, with the specific function or struct
- Any new files to create
- Which existing skill's checklist to follow (if applicable)
- Unit tests and integration tests that will verify the change
- README sections that need updating
