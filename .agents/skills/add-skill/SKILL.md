---
name: add-skill
description: Create a new project skill in .agents/skills/ and register it in the skills README
argument-hint: "<skill-name> <one-line description>"
---

# Add Skill

Create a new project-specific skill in `.agents/skills/`.

## Description

Skills are instruction documents that Claude follows when invoked with `/skill-name`. Each skill lives in its own directory under `.agents/skills/` and is described by a single `SKILL.md` file with YAML frontmatter.

After creating the skill, register it in `.agents/skills/README.md`.

## File structure

```text
.agents/skills/
├── README.md                  ← index of all skills (update this)
└── <skill-name>/
    └── SKILL.md               ← the skill definition
```

## `SKILL.md` format

Every `SKILL.md` starts with YAML frontmatter, followed by a markdown body.

### Frontmatter

```markdown
---
name: skill-name
description: One-line description shown in the skill list — be specific about what the skill does
argument-hint: "<required-arg> [optional-arg]"   # omit if the skill takes no arguments
---
```

- **`name`** — kebab-case, matches the directory name exactly.
- **`description`** — shown in the invocation list; write it from the user's perspective ("Run…", "Add…", "Create…"). Aim for one concrete sentence; avoid vague terms like "helper" or "utility".
- **`argument-hint`** — optional. Use angle brackets for required args, square brackets for optional ones.

### Body structure

```markdown
# Title

One-sentence summary of what the skill does.

## Description

When to invoke this skill and why. Include any prerequisites or context
the reader needs before following the instructions.

## Instructions

Step-by-step instructions Claude will follow when the skill is invoked.
Write in the imperative ("Run…", "Add…", "Check…"). Be concrete: name
exact files, commands, and patterns rather than describing them in the
abstract.

Use `### Step N — name` headings for multi-step workflows. Include
command blocks, code examples, and a final checklist when appropriate.
```

### What makes a good skill

- **Concrete over abstract** — name the exact file, function, or command. "Add a field to the `Cli` struct in `src/main.rs`" beats "modify the argument parser".
- **Show the pattern** — include a code block with the exact shape of what needs to be written, using `myname` / `MY_VALUE` placeholders.
- **Cover the full surface** — think about every file or system that changes: source code, tests, documentation, config. A skill that omits one location will reliably produce incomplete results.
- **Checklist at the end** — for multi-step authoring skills, close with a `## Checklist` of `- [ ]` items so nothing is forgotten.
- **Short description** — the `description` frontmatter is read without the body; make it self-contained.

## Instructions

### Step 1 — choose the skill name

Use kebab-case. The name is the slash-command users will type (`/my-skill`), so it should be short, unambiguous, and action-oriented: `check`, `add-base-image`, `integration-tests`.

### Step 2 — create the directory and file

```bash
mkdir -p .agents/skills/<skill-name>
```

Then write `.agents/skills/<skill-name>/SKILL.md` following the format above.

### Step 3 — write the frontmatter

Fill in `name` (must match the directory name), `description`, and `argument-hint` if the skill accepts arguments.

### Step 4 — write the body

Open with a `# Title` heading and a one-sentence summary. Then add:

- **`## Description`** — when and why to use the skill, any prerequisites.
- **`## Instructions`** — the step-by-step content Claude will execute. For skills that guide a multi-location change (like `add-cli-flag` or `add-base-image`), number the steps and close with a `## Checklist`.

Reference existing skills in `.agents/skills/` as models — `check` for a simple execution skill, `add-cli-flag` or `add-base-image` for a multi-step authoring checklist.

### Step 5 — register in the README

Add a row to the table in `.agents/skills/README.md`:

```markdown
| [`skill-name`](skill-name/SKILL.md) | One-line description matching the frontmatter `description` field |
```

Insert it in the same logical grouping as related skills (everyday tasks before authoring tasks).
