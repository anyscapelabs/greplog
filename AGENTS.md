# AGENTS.md — Rules for AI Coding Agents Working on Greplog

This file is read by any AI coding agent (Claude Code, or equivalent) before
making changes to this repo. These are hard constraints, not style
preferences. If a rule below conflicts with completing a task the way it was
asked, stop and flag the conflict — do not silently work around a rule to
make a task "pass."

---

## Rule 1: Zero warnings, zero errors, always

Before considering any change complete:

- `cargo build --workspace` and `cargo test --workspace` must produce **zero
  warnings and zero errors** — not "warnings that don't matter," not
  "pre-existing warnings I didn't introduce." If a change surfaces a
  pre-existing warning, fix it as part of the change or flag it explicitly
  rather than leaving it.
- `cargo clippy --workspace --all-targets -- -D warnings` must pass clean.
  Treat clippy warnings as build errors, not suggestions.
- For SDKs: the equivalent per-language lint/typecheck must pass clean
  (`eslint`/`tsc --noEmit` for Node, `mypy`/`ruff` for Python, `go vet` +
  `golangci-lint` for Go).
- Do not use `#[allow(...)]`, `// eslint-disable`, `# noqa`, or equivalent
  suppressions to silence a warning instead of fixing it, unless there is a
  specific, commented reason the warning is a false positive in that exact
  spot. A suppression with no comment explaining why is not acceptable.

**A change is not done if it introduces a new warning anywhere in the
workspace, even in a file you didn't intend to touch.** Run the full
workspace build/test/lint, not just the package you edited.

---

## Rule 2: No `panic!`, `unwrap()`, or `expect()` in non-test code

This applies to every Rust crate (`core`, `agent`, `cli`) and the equivalent
in every SDK.

- **Production code paths must handle errors explicitly** — `Result`,
  `Option` with real handling, or an explicit, justified early return. No
  `.unwrap()`, `.expect(...)`, or bare `panic!()` in any file under `src/`
  in any crate or SDK.
- **`tests/` and `#[cfg(test)]` modules are exempt.** Panicking on an
  unexpected condition in a test is correct and expected — that's what makes
  a test fail loudly. Do not weaken test code to avoid `unwrap()` there;
  the rule is about production paths only.
- **If a "this should never happen" case needs handling**, use
  `unreachable!()` only when it is genuinely provably unreachable (and
  comment why), or return a proper error type. Do not reach for `unwrap()`
  as a shortcut past a `Result` you don't want to handle right now.
- Before finishing any change, grep the diff for `unwrap()`, `expect(`,
  and `panic!(` outside of `tests/` and `#[cfg(test)]` blocks. If any are
  found in a file you touched, either remove them or justify explicitly why
  this specific instance is provably safe — "it should be fine" is not a
  justification.
- Same principle applies per SDK: no unhandled exceptions thrown from a
  code path a host app can trigger (Node: no uncaught throw from SDK-owned
  code; Python: no bare `raise` from SDK internals reaching the host app;
  Go: no `panic()` outside test code). This is also required by the
  fail-open guarantee in `docs/adr/0009-sdk-startup-fail-open.md` — an SDK
  that panics the host app violates that ADR, not just this rule.

---

## Rule 3: Production and test code never share a file or folder

- `src/` and `tests/` (or the language-idiomatic equivalent — `test/` for
  Node, etc.) are always siblings, never interleaved. Do not add
  `#[cfg(test)] mod tests { ... }` inline at the bottom of a production file
  as a default — prefer a sibling file in `tests/` for anything beyond a
  small, tightly-coupled unit test of a private function. If a private
  function genuinely needs a same-file test (because it's not
  publicly reachable), keep that inline test minimal and put integration-
  level tests in `tests/` regardless.
- Never commit a file that mixes example/scratch code with production logic.
  If you generated exploratory code to verify an approach, delete it before
  finishing, or move it to a clearly-named `examples/` folder if it has
  lasting value.

---

## Rule 4: Lock ordering and concurrency invariants (agent crate specifically)

These are load-bearing correctness properties, established across Rounds
9-11 of this project's investigation. Do not change them without flagging
it first, even if a change looks like a harmless refactor:

- **Never acquire `wal_lock` while holding `buffer_lock`.** The only allowed
  order is: acquire `buffer_lock` → release → acquire `wal_lock` → release →
  acquire `buffer_lock` again for insert. This order avoids deadlock with the
  flush path, which takes the same ordering.
- **Never hold a lock across `.await`**, and never hold a lock across a
  blocking syscall in `spawn_blocking` code without confirming that's the
  intended holder (the WAL lock during `write_all`/`write_vectored` is the
  one intentional exception).
- **Capacity checks and buffer inserts must be atomic per-entry**, evaluated
  in arrival order, under a single continuous lock hold for a batch — never
  check capacity for a whole group before any insert happens (this exact bug
  was found and fixed in Round 11; do not reintroduce it).
- **Do not add new locks, atomics, or channels** to solve a problem without
  flagging it first. Every synchronization primitive currently in this
  codebase has a specific, documented reason. An unreviewed addition is the
  most likely way to reintroduce a deadlock or a hidden serialization point.
- **Replay logic must remain deterministic.** If a change affects WAL
  replay, capacity must be re-derived from a segment-persisted value, not
  from live config, and rejected-on-replay entries are not errors — they are
  expected, not data loss.

---

## Rule 5: Documentation changes follow `docs/STYLE_GUIDE.md`

If a code change affects behavior described in any doc under `docs/`:

- Update the doc in the same change, not as a follow-up. A behavior change
  merged without its doc update is incomplete, not "done with docs pending."
- Every feature/component doc carries a status marker (✅ Shipped / 🚧 In
  progress / 📋 Planned) — update it based on the actual code state you just
  produced, not from memory of what was planned.
- Before finishing, grep `docs/` for the subsystem name you touched and fix
  any other doc that's now stale, per `docs/STYLE_GUIDE.md` §4. Do not leave
  two descriptions of the same subsystem in different states.
- Roadmap/status changes go in `ROADMAP.md` at the repo root — never inline
  inside an architecture or implementation doc.

---

## Rule 6: Don't touch performance-frozen paths without flagging it

Per the current project phase, throughput/performance work on the ingest
path is intentionally frozen until after v0.1 adoption (see `ROADMAP.md` and
`docs/adr/` for the specific decisions this affects — WAL batching,
pre-lock parallelization, etc.). If a task seems to require touching these
paths for a reason other than a correctness fix:

- Stop and flag it rather than proceeding, even if the change looks like an
  obvious improvement.
- Correctness fixes to these paths (e.g., the capacity-check ordering bug)
  are never frozen — the freeze applies to speed, not to correctness.

---

## Rule 7: When a rule conflicts with a test passing

If following any rule above would make a test fail, or a task ask for
something that conflicts with a rule above:

- **The rule wins.** Report the conflict clearly, rather than relaxing a
  rule to make a test green or a task technically complete.
- This usually means the test's assumptions are stale (e.g., a test written
  before a capacity-check fix, still asserting the old, buggy behavior) —
  not that the invariant is wrong. Flag it as such rather than quietly
  adjusting the invariant to match a stale test.

---

## Quick pre-completion checklist for any agent finishing a task here

- [ ] `cargo build --workspace` / lint equivalents: zero warnings, zero errors
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] No `unwrap()` / `expect()` / `panic!()` introduced outside `tests/` or
      `#[cfg(test)]`
- [ ] No production/test code mixed in the same file or folder
- [ ] Lock ordering (`buffer_lock` → `wal_lock` → `buffer_lock`, never
      `wal_lock` held with `buffer_lock`) unchanged, or flagged if it had to
      change
- [ ] Any behavior change has its doc updated in the same commit, status
      marker current, `docs/` grepped for staleness elsewhere
- [ ] Roadmap changes in `ROADMAP.md`, not buried in an architecture doc
- [ ] No performance-frozen path touched except for a correctness fix
- [ ] Full workspace test suite run, not just the package touched
