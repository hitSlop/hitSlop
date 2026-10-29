# Authoring workflow

**Do not create an automated test or test suite for each tiny.** The checks and native
review below validate the work without adding example-specific tests. Shared SDK,
storage, or host regressions belong in existing tests at their owning boundary.

Read tiny.ts, AGENTS.md, and BRIEF.md, then edit tiny.ts, schema.ts, and the UI source.
Run the generated project's `bun run check` and `bun run dev`; refresh resets
preview state. Vite watches source: component/CSS updates keep accepted edits, while a change to tiny.ts, schema.ts, commands.ts, their dependencies or the root role inventory resets disposable state. In the repository,
use `bun tiny COMMAND SOURCE`.

Run `bun run build`, then `bun run register`; macOS and Linux render the preview and icon artwork
you do not supply. Pack also renders every declared view, including Export PNG/PDF
and all Widget families, even with supplied artwork; any view failure prevents publication.
Each static role imports its own stylesheets in import order; `tiny dev` still mounts
static views inside the editor page. Create a writable copy in the
Mac app's catalog. Manually review close-after-type, reopen,
keyboard/IME, themes, duplication, and PNG/PDF. Inspect preview and icon artwork.
Never edit a runtime master or publish private document state as a template.
Hosted template publication is deferred; npm distributes the authoring tools.
