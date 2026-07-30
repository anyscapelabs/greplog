# Greplog Documentation Style Guide

This is the canonical reference for how to write any doc in this repo. If
you're adding or editing anything under `docs/`, `ROADMAP.md`, or a
per-package `README.md`, read this first. It exists because every rule here
maps to a real mistake found and fixed in Round 13 — this isn't generic
advice, it's a checklist against known failure modes in this exact project.

---

## 1. Which folder does this doc belong in?

| If you're writing about... | It goes in... |
|---|---|
| How to install and get a dashboard running | `docs/quickstart.md` |
| How a subsystem works internally (WAL, dedup, buffer, flush, compaction, query) | `docs/architecture/agent-pipeline.md` — **do not create a second file describing this pipeline anywhere else.** Link to it. |
| The system-wide map (how agent/SDKs/dashboard/CLI relate) | `docs/architecture/overview.md` |
| Dashboard UI/UX behavior | `docs/architecture/dashboard.md` |
| CLI commands and flags | `docs/architecture/cli.md` — this is the **only** CLI reference. `implementation-v0.1.md` and any other doc must link here, not re-list commands. |
| A locked-in decision with tradeoffs | a new numbered file in `docs/adr/` |
| SDK shared contract (transport, redaction, manual API) across all languages | `docs/sdk/design.md` |
| SDK self-detection (a language SDK detecting its own framework at init) | `docs/sdk/auto-detection.md` — **note: this is not the same mechanism as the agent's workspace-level `/detect` endpoint.** If you're documenting the agent's workspace detection instead, that belongs in `docs/architecture/overview.md` or a dedicated agent doc, and must say explicitly that it's different from SDK self-detection. |
| Per-language SDK reference (function signatures, config options) | `docs/sdk/<language>.md` — prefer generating this from code (rustdoc/Sphinx/TypeDoc/godoc) over hand-writing it |
| Binary/package distribution mechanics | `docs/distribution/` |
| What's done vs. planned, by component | `ROADMAP.md` at repo root — **never inside an architecture or implementation doc** |
| A single package's build/test instructions | that package's own `README.md` (see §6) |

If you're unsure which folder a doc belongs in, that's a signal to search
`docs/` for the subsystem name first (§4) — the right home is often already
implied by an existing doc you're about to accidentally duplicate.

---

## 2. Every doc needs a status marker

Any doc describing a feature, component, or behavior — not a pure how-to —
starts with:

```markdown
> **Status:** ✅ Shipped (v0.1) | 🚧 In progress | 📋 Planned, not started
```

If a single doc covers multiple pieces at different stages (e.g. a dashboard
doc where some views are built and some aren't), mark **each section**
individually rather than picking one marker for the whole file:

```markdown
## Interleaved Log Explorer
> **Status:** ✅ Shipped (v0.1)
...

## Saved Views
> **Status:** 📋 Planned
...
```

**Before adding or changing a status marker, check the actual code or a
recent test run — never carry a marker forward from memory of what was
planned.** A wrong status marker is worse than none: it actively misleads
instead of leaving an honest gap. This single rule would have caught both
the `IngestResponse` contradiction and the dashboard-status mismatch found in
Round 13.

---

## 3. Tense and voice rules (don't let grammar imply status)

- A shipped feature is described in **present tense, stated as fact**: "The
  agent writes each batch to the WAL before acknowledgment."
- A planned feature is described with **future tense or an explicit planned
  marker**, never present-tense fact: "The dashboard will support saved
  views" or "📋 Planned: saved views," never "The dashboard supports saved
  views" for something still on mock data.
- Do not rely on tense alone to carry this distinction — pair it with the
  status marker from §2 every time. Tense is easy to slip on during editing;
  the marker is the thing a reviewer actually checks.

---

## 4. Before you submit: the duplication check

Before committing any doc change, **grep the rest of `docs/` for the
subsystem name you just wrote about.** This is the actual mechanical habit,
not a suggestion:

```bash
grep -ril "compaction" docs/
grep -ril "dedup" docs/
```

If you find a hit outside the file you're editing, one of two things is true:
1. You're duplicating a description that already has a canonical home —
   delete your version, link to the canonical doc instead, and write at most
   a one-paragraph summary of why it's relevant in the new location.
2. The other doc is now stale relative to what you just wrote — fix it in
   the same commit, don't leave two versions to drift apart. This is exactly
   how the `IngestResponse` "not yet sent" vs. "Done" contradiction happened:
   two true-at-the-time statements, in two places, and only one got updated.

---

## 5. ADR template — required for every entry in `docs/adr/`

```markdown
# ADR-00XX: <Decision Title>

**Status:** Accepted | Superseded by ADR-XXXX | Deprecated
**Date:** YYYY-MM-DD

## Context
<What problem forced this decision — 2-4 sentences>

## Decision
<The actual decision, stated plainly>

## Alternatives considered
<What else was on the table and why it lost — even one line per alternative>

## Consequences
<What this makes easier, what it makes harder or forecloses>
```

- **Every ADR gets a date.** An undated decision reads as permanent and
  discourages the "should we revisit this" conversation a growing OSS
  project needs.
- **A superseded ADR is never deleted** — mark its status as `Superseded by
  ADR-XXXX` and add the new ADR. History is the point.
- New architectural decisions get a new numbered file, sequential, never
  reusing or renumbering existing entries.

---

## 6. Package README template — required for every crate/SDK/package folder

```markdown
# <package name>

<one-sentence description of what this package is>

## What this is
<2-4 sentences: role in the system, what it does not do>

## Building
<language-specific build command(s)>

## Testing
<how to run this package's tests specifically>

## Structure
src/    — <brief note on what lives here>
tests/  — <brief note on what lives here>

## Relationship to the rest of greplog
<1-2 sentences + link to docs/architecture/overview.md>
```

Same shape everywhere, regardless of language — a contributor opening any
package folder should recognize the structure immediately. The root
`README.md` is the only place that describes overall system architecture in
full; package READMEs link out to it rather than re-explaining it.

---

## 7. Roadmap entries

`ROADMAP.md` is organized by component (agent, cli, core, dashboard, sdk/*),
each item as:

```markdown
| Feature | Priority | Status | Description |
|---|---|---|---|
| Metric aggregation & rollups | Medium | 📋 Planned | Pre-compute time-bucketed aggregates for dashboard charts |
```

Roadmap content never lives inside an architecture or implementation doc —
if you're writing a "what's left" section anywhere else, move it here
instead of leaving it where you happened to be editing.

---

## 8. Comparisons and benchmarks in any public-facing doc

Do not include competitor comparisons or head-to-head benchmark claims in
`docs/quickstart.md`, the root `README.md`, or any launch-facing copy unless:

- the benchmark methodology is published and reproducible (a script in
  `bench/`, hardware/config stated), **and**
- the numbers reflect the current, stable perf baseline — not a
  best-case or cherry-picked scenario.

Until both are true, describe scope and known limitations plainly instead
(e.g. "single-node, local-first; Windows requires WSL2 — see ADR-0002") and
avoid superlative language ("fastest," "lowest overhead") entirely. Honest
scope reads as more trustworthy at launch than an unscoped claim invites
scrutiny it can't yet survive.

---

## 9. Quick checklist before committing any doc change

- [ ] Is this in the right folder per §1, or duplicating something that
      already has a canonical home?
- [ ] Does every feature/component description have a current, code-checked
      status marker (§2)?
- [ ] Does tense match status — no present-tense claims for planned work (§3)?
- [ ] Did you grep `docs/` for this subsystem name and fix any other doc
      that's now stale (§4)?
- [ ] If this is an ADR, does it have a date and follow the template (§5)?
- [ ] If this is a package README, does it follow the shared template and
      avoid re-explaining the whole system (§6)?
- [ ] Is any "what's left" content in `ROADMAP.md`, not buried here (§7)?
- [ ] No unpublished-benchmark comparison claims snuck into launch copy (§8)?
