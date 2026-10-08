# Authoring workflow

**Do not create an automated test or test suite for each slop.** The checks and native
review below validate the work without adding example-specific tests. Shared SDK,
storage, or host regressions belong in existing tests at their owning boundary.

Read slop.ts, AGENTS.md, and BRIEF.md, then edit slop.ts, schema.ts, and the UI source.
Run the generated project's `bun run check` and `bun run dev`; refresh resets
preview state. Vite watches source: component/CSS updates keep accepted edits, while a change to slop.ts or any non-component module it imports (document, commands, skin, artwork) resets disposable state. In the repository,
use `bun slop COMMAND SOURCE`.

Run `bun run build` (any platform), then `bun run register` on a Mac with a compatible installed app.
Create a writable copy in the native catalog. Manually review close-after-type, reopen,
keyboard/IME, themes, duplication, and PNG/PDF. Inspect preview and icon artwork.
Never edit a runtime master or publish private document state as a template.
Hosted template publication is deferred; npm distributes the authoring tools.
