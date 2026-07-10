# Story e01s01: Restyle all HTML templates with Pico CSS

**type:** refactor
**risk:** P2
**context:** ui

## §1 Business Narrative

Papyrd's UI currently relies on ~500 lines of bespoke dark-theme CSS with class-heavy
markup across 10 HTML templates. This adds maintenance overhead and makes it harder
to add new pages consistently. Switching to Pico CSS v2 — a class-light semantic CSS
framework — cuts CSS maintenance, makes markup cleaner, and provides built-in
dark/light mode support.

## §2 Actors

- **All users** — see the restyled UI on every page
- **Developers** — benefit from simpler templates and less CSS to maintain

## §3 Trigger

User navigates to any Papyrd page. The page renders with Pico CSS styling via
semantic HTML elements and minimal Pico utility classes.

## §4 Preconditions

- Pico CSS v2 files (`pico.min.css`, `pico.colors.min.css`) present in `static/`
- `layout.html` links to Pico CSS instead of old `app.css`
- Server built successfully with updated Askama templates

## §5 Main Flow

1. Request arrives at any Papyrd route
2. Askama renders the template with Pico-compatible HTML structure
3. Browser applies Pico CSS to semantic elements (header, nav, main, article, form, etc.)
4. Additional styling from `app.css` applies only to Pico-unmatched visuals
   (book cover cards, cover fallback gradients)
5. Dark/light mode follows OS preference via `<meta name="color-scheme">`

## §6 Alternative Flows

- **Offline browsing:** Pico CSS is self-hosted — no CDN dependency.
  Styling works without internet access.
- **OPDS XML:** OPDS templates are XML, not HTML — Pico has no effect on them.

## §7 Error States

- **CSS missing:** If `pico.min.css` is missing, page renders unstyled (same as
  if any CSS file were missing). No functional impact.
- **Template syntax error:** Askama compile error caught by `cargo build`.

## §8 Postconditions

- All 10 HTML page templates use Pico-compatible semantic HTML
- `static/app.css` is minimal (~50 lines or fewer for cover cards only)
- `cargo build` and `cargo test` pass
- Every page renders with Pico styling applied

## §9 Non-Functional Requirements

- No CDN dependency — CSS files self-hosted in `static/`
- No JavaScript added
- No change to page load performance (Pico CSS is ~20KB minified)

## §10 Dependencies

- Pico CSS v2 (self-hosted, not a Cargo dependency)

## §11 Assumptions

- Pico CSS v2 is stable and sufficient for all current layouts
- Grid layout needed for book cards — using Pico's `.grid` class
- Cover fallback gradients require minimal custom CSS

## §12 Out of Scope

- OPDS XML templates (opds_v1/*.xml)
- Rust handlers, routes, database
- Interactive JS features
- Dark/light mode toggle (OS preference only)

## §13 Risks

- Pico's default styling may not match current visual density (book cards, forms)
- Mitigation: review each page visually after restyle; write minimal custom CSS
  only where Pico falls short

## §14 References

- Pico CSS docs: <https://picocss.com/docs>
- `specs/planning-context.yaml` — elaborated spec with key decisions

## §15 Definitions

- **Pico CSS:** Class-light CSS framework that auto-styles semantic HTML
- **Class-light:** Uses `.container`, `.grid`, and component classes for layout;
  native elements (forms, buttons, tables, nav) styled automatically

## §16 Test Cases

- `cargo build` — verifies all Askama templates compile
- `cargo test` — verifies no handler behavior regression
- Manual visual inspection of each page in light and dark modes

## §17 Acceptance Criteria (Gherkin)

```gherkin
Given the server is running
When I navigate to any page (books, book detail, authors, author detail,
  signin, signup, upload, admin users, admin permissions, admin invites)
Then the page renders with Pico CSS semantic styling applied
And no custom CSS classes from the old app.css remain in templates
And the page is usable in both light and dark OS modes

Given the server is built
When I run cargo test
Then all existing tests pass

Given the static/ directory
When I inspect app.css
Then it contains only cover fallback and card layout styles
And is substantially smaller than the original ~500 lines
```

## §18 What This Story Is NOT

- NOT a behavior change — no new features, no handler modifications
- NOT an OPDS protocol change — XML templates untouched
- NOT a JavaScript enhancement — no interactivity added
- NOT a design system — Pico provides the design system; we adapt to it
