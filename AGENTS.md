<!-- BEGIN bigpowers:project -->
# Papyrd — AI Agents

Read CONVENTIONS.md before any GitHub or git operation.

## Project

Papyrd is a self-hosted eBook server implementing OPDS for eBook discovery/download and Kosync for reading progress sync.
Stack: Rust / Axum / sqlx (Postgres) / Tokio / Askama

## Commands

| Action | Command |
|--------|---------|
| Run    | `cargo run` |
| Test   | `cargo test` |
| Build  | `cargo build` |
| Lint   | `cargo clippy` |
| Preflight | `cargo test && cargo build` |
| CI     | `gh pr checks` (when a PR is open) |

## Architecture

Axum handlers route HTTP requests into repository-layer database calls via sqlx; Askama templates render HTML responses. A separate ingest binary processes EPUB files dropped into a watched directory. Domain modules (user, reading_progress) sit between handlers and repositories. OPDS v1/v2 and Kosync protocol handlers live alongside the web UI handlers.

## Conventions

- Keep things simple — don't overabstract early. Duplication is fine if it keeps handlers flat.
- Render templates inline in the handler. Don't add tiny `render_*` or `response_*` helpers.
- Prefer clear procedural code in handlers over "clean architecture".
- Most routes are simple CRUD — let logic live in handlers unless directed otherwise.

## Never

- Never dismiss reproducible gate failures as pre-existing or out of scope
- Never proceed on red Preflight or red CI — invoke quick-fix or fix-bug first
- Never push to main
- Always ask before squashing/merging
- Always do work on a separate branch (never directly on main)

## Agent Rules

- **Workflow Mandate:** You MUST use the bigpowers skills (e.g. `plan-work`, `develop-tdd`, `orchestrate-project`) to perform tasks. DO NOT write code directly in response to a user prompt like "build this feature".
- **Always Green:** Preflight and CI must be green before forward work. Reproducible gate failures require **fix-or-log** (quick-fix → fix-bug) per CONVENTIONS § Discovered Defects.
- Read specs/ before writing code.
- All planning and specifications MUST be written to `specs/` (`product/SCOPE_LATEST.yaml`, `release-plan.yaml`, `epics/`) before any code is generated.
- Write the minimum code that solves the stated problem. Nothing extra.
- Run tests after every change. Show evidence before declaring done.
- One clarifying question beats a wrong assumption baked into 200 lines.
<!-- END bigpowers:project -->

This project is a Rust web server using the Axum web framework, and sqlx for postgres interactions.
The goal of this project is to be a web server targeted at the self hosted/homelab audience.
It will be used to host eBooks and implement OPDS and Kosync protocols so that clients
can plug into this server.

## Code architecture

When coding and refactoring keep things simple.
Don't overabstract things early on. It's ok if some handlers have some duplication.
The ideal code flow for any given route is simply

1. Validate input
2. Call a database function to save/load data
3. Optionally call some other business logic if needed
4. Serialize response

Prefer clear, flat handlers even if that means duplicating a few lines. Render templates inline
in the handler instead of adding tiny `render_*`, `response_*`, or similar helpers whose only
job is to construct a template, call `.render()?`, wrap it in `Html`, attach a status, or return
a redirect. This applies even when the helper would have two call sites or several validation
branches.

Most of our routes are going to be simple CRUD apps and most logic can live in handlers.
I will direct you on if and when we need to abstract out some trickier business logic.
Otherwise prefer clear direct procedural code in handlers over 'clean architecture'.

<!-- BEGIN bigpowers:context-routing -->
<!-- No sub-AGENTS.md files configured yet. -->
<!-- END bigpowers:context-routing -->

<!-- BEGIN bigpowers:learned-preferences -->
## Learned User Preferences
<!-- Preferences discovered during sessions will be recorded here. -->

## Workspace Facts
<!-- Workspace-specific knowledge will be recorded here. -->
<!-- END bigpowers:learned-preferences -->

<!-- BEGIN bigpowers:tooling -->
<!-- Tooling blocks (sqz/rtk/hooks) will be installed by setup-environment and guard-git. -->
<!-- END bigpowers:tooling -->
