# rgctl agent pack (install)

rgctl has two surfaces:

| Layer | What it is | Where it lives |
|--------|------------|----------------|
| **Engine** | `rgctl discover`, `rgctl -f json find|callers|…`, … | Your PATH; graph artifacts in `{repo}/.rgctl/` |
| **Agent pack** | Meta skill + **seven workflow skills** + optional policy | Repo-local agent dirs (or home with `-g`) |

The pack is **embedded in the `rgctl` binary** (no separate download). Install the CLI first: [Installation](../installation.md).

`--skill` installs the meta router and seven workflow skills. Agents load those skills and run structured CLI verbs — there are no slash-command / prompt stubs.

Workflow text is authored under **`skills/rgctl/workflows/`**; `references/workflows.md` in the installed meta skill is **assembled at build time** from those fragments (single source of truth).

---

## Command shape

```bash
rgctl [-r REPO] install [FLAGS]
```

- **`-r REPO`** — Repository root where agent directories are written (default: current working directory). Use the same root as `discover`.
- **`install`** — Copies files from the embedded agent pack.

You must pass at least one of **`--skill`** or **`--with-policy`**.

### Flags

| Flag | Effect |
|------|--------|
| **`--skill`** | Meta skill **`rgctl`** (router + `references/`) and seven workflow skills: `rgctl-discover`, `rgctl-impact`, `rgctl-flow`, `rgctl-search`, `rgctl-migrate`, `rgctl-kantra`, `rgctl-gate`. |
| **`--with-policy`** | Structural bias snippet (e.g. `.cursor/rules/rgctl-structural.mdc`). Optional; does not replace skills. |
| **`--tools id1,id2`** or **`--tools all`** | Which **registry adapters** receive files. **Default (omit flag):** `cursor`, `claude`, `codex`, `agents`, `antigravity`. **`all`** = full registry (~40 products). Unknown ids: stderr warning; if none valid, exit **1**. |
| **`-g` / `--global`** | Install under your **home** (e.g. `~/.cursor/skills/…`) instead of repo-local paths. Only agents with `supports_global: true` in the registry (see `--list-agents`). |
| **`--list-agents`** | Print the registry table and exit (no install). |
| **`--force`** | Overwrite rgctl-managed files that differ from the bundled version. |
| **`--host`** | **Deprecated** — use **`--tools`**. |

### Typical installs

```bash
cd /path/to/your-app

# Skills for common IDEs (repo-local)
rgctl install --skill --tools cursor,claude,codex,antigravity,agents

# Cursor only
rgctl install --skill --tools cursor

# Antigravity (`.agent/skills/`)
rgctl install --skill --tools antigravity

# Every registry adapter
rgctl install --skill --tools all

# Skills + Cursor structural policy
rgctl install --skill --tools cursor --with-policy

# User-home install (adapters that support -g)
rgctl install --skill -g --tools cursor
```

Install does **not** run `discover`. Index the codebase separately:

```bash
export REPO=/path/to/your-app
cd "$REPO"
rgctl discover .
# then: rgctl -f json find|callers|relations|…
```

Add `.rgctl/` and agent skill dirs to `.gitignore` if you want them local-only.

### Upgrades

1. Install a newer **`rgctl`** binary.
2. Re-run install with **`--force`** if managed files already exist and differ.

---

## What gets written

Paths come from **`agent-pack/agents/registry.toml`** (per-product `agent_dir`, `skills_subdir`).

| Kind | Example (Cursor, repo-local) |
|------|------------------------------|
| Meta skill | `.cursor/skills/rgctl/SKILL.md` + `references/` |
| Workflow skill | `.cursor/skills/rgctl-discover/SKILL.md`, … |
| Policy | `.cursor/rules/rgctl-structural.mdc` (with `--with-policy`) |

**Shared dedup:** **`codex`**, **`agents`**, and **`zed`** use **`.agents/skills/`**; install writes each destination once.

### Adapter examples

| Agent | Skills |
|-------|--------|
| Cursor | `.cursor/skills/rgctl-*` |
| Claude | `.claude/skills/rgctl-*` |
| OpenCode | `.opencode/skills/rgctl-*` |
| Pi | `.pi/skills/rgctl-*` |
| GitHub Copilot | `.github/skills/` |

Run **`rgctl install --list-agents`** for the full table.

---

## Workflows → CLI

| Workflow | Skill | Primary CLI |
|----------|-------|-------------|
| Index | `rgctl-discover` | `discover` |
| Impact | `rgctl-impact` | `blast-radius` |
| Data flow | `rgctl-flow` | `slice`, `cpg flows`, … |
| Search | `rgctl-search` | `find`, `semantic query`, … |
| Migration roadmap | `rgctl-migrate` | discover + `migration_plan.json` |
| Kantra rules | `rgctl-kantra` | `discover --with-kantra` |
| CI gate | `rgctl-gate` | `check`, `pr-check` |

**Migrate** (roadmap / `migration_plan.json`) and **Kantra** (Konveyor / `kantra_findings.json`) are **separate** workflows — do not conflate them in prompts or reports.

---

## JSON output

```bash
rgctl -r "$REPO" -f json install --skill --tools cursor
```

Uses **`schema_version`: 3** (`scope`, `agents`, `with_policy`, per-write `agent`, `workflow`, `kind`, `status`). See [JSON API §18](../json-api.md#18-install). If any write is `skipped_exists`, JSON is still printed and the process exits **1**.

---

## Structural policy (optional)

```bash
rgctl install --with-policy --tools cursor
```

**Cursor-only today:** writes `.cursor/rules/rgctl-structural.mdc` regardless of `--tools` (other agents have no policy adapter yet). Best-effort nudge toward `rgctl -f json` when `.rgctl/` exists; agents cannot hard-block grep.

### AGENTS.md for *your* repo

Prefer **`rgctl install --skill`**. If you skip `--with-policy` / skills, paste either:

- the short nudge below into your repo’s root **`AGENTS.md`**, or
- the full playbook from [USER_AGENTS_TEMPLATE.md](../agents/USER_AGENTS_TEMPLATE.md)

```markdown
When `.rgctl/` exists, answer structural questions via `rgctl -f json` before ripgrep or bulk file reads.
```

The **rgctl source tree** root [`AGENTS.md`](../../AGENTS.md) is for **contributing to rgctl** (not a consumer CLI cookbook).

---

## Related

- [Agent pack walkthrough](agent-skill.md) — use cases and agent loop
- [USER_AGENTS_TEMPLATE](../agents/USER_AGENTS_TEMPLATE.md) — paste into consumer repos
- [JSON API §18](../json-api.md#18-install) — install schema
- [Agent pack release notes](../releases/agent-pack-install.md)
