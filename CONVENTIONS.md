# Papyrd — Conventions

Shared rules for all AI agents working on this project.

---

## § Always Green / Shift Left

The cheapest bug is the one never written. The next cheapest is the one caught before commit. The most expensive is the one in production (1-10-100 rule).

- **Preflight** (`cargo test && cargo build`) must pass before any commit.
- **CI** must be green before merging. A red CI means stop forward work and fix it.
- Every change must pass all existing tests. If a test breaks, fix the code or update the test — don't skip.

## § Discovered Defects

When you discover a defect unrelated to your current task:

| Action | When |
| -------- | ------ |
| **quick-fix** | Trivial fix (< 5 lines, data-only, no logic change) — fix in its own commit |
| **fix-bug** | Requires investigation — write `specs/bugs/BUG-<slug>.md`, fix in separate PR/branch |
| **Log it** | Cosmetic or too large to address now — file an issue and note it in commit message |

Never silently fix a discovered defect in the same commit as feature work. Separate commits always.

## Banned Dismissive Phrases

The following phrases are never acceptable when a gate is red:

| Phrase | Why it's banned |
| -------- | ----------------- |
| "pre-existing" | Doesn't matter — red gate blocks forward work regardless of origin |
| "unrelated to my session" | If it's on the branch, it's your problem |
| "not introduced by my changes" | A red gate is a red gate. Fix or log it. |
| "out of scope" | Ignoring a red gate is always out of scope |

## Defensive Code

No additional defensive code categories specified beyond what's already implemented in the project.

## Specs Output Convention

All planning artifacts live in `specs/`:

```
specs/
├── product/           # Scope, vision, glossary
├── tech-architecture/ # Tech stack, security, test, design, refactor, impact plans
├── epics/             # Active epics and archive
├── adr/               # Architecture Decision Records
├── verifications/     # Verification plans and results
├── bugs/              # Bug reports and registry
├── release-plan.yaml  # WSJF-ordered epic sequence
├── state.yaml         # Session state and workflow mode
├── execution-status.yaml
└── planning-status.yaml
```

---

## Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]
```

Types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`, `ci`, `perf`

Example: `feat(handlers): add OPDS v2 navigation feed`
