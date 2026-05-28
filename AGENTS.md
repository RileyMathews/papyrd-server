# Papyrd
This project is a Rust web server using the Axum web framework, and sqlx for postgres interactions.
The goal of this project is to be a web server targeted at the self hosted/homelab audience.
It will be used to host eBooks and implement OPDS and Kosync protocols so that clients
can plug into this server.

# Code architecture
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
