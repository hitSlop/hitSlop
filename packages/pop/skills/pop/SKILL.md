---
name: pop
description: Create, fill, open, and export useful pop documents, or build a new mini app when no template fits.
---

# Make a local pop document

Do not display “Saved,” “Saving…,” or routine persistence indicators inside authored pop. The native host owns save-failure and retry UI. Use task-specific feedback for explicit operations, such as “Importing skin…” or “Skin applied.”

Use `bunx @hitslop/pop`, or install with `bun install -g @hitslop/pop` and use `pop`; both need Bun. `pop` uses the engine bundled with its npm package, which on macOS and Linux also renders template artwork and a closed document's PNG/PDF export itself; `pop open` needs the installed Mac app, and the bundled starters in `pop templates` come from it. `pop templates` lists the templates the Mac catalog shows as JSON: each one's slug, title, description, categories, source (`bundled` with the app, or `installed` under ~/.pop/templates) and path. People browse the same catalog with File → New from Template. Start from a listed template that fits; when none does, read pop-authoring and pop-design for new source. Local templates and recents work offline. Remote catalog loading, publication, and sharing are deferred.

Make a writable document with `pop create --from SLUG --output PATH` at a fresh user-selected path, where SLUG comes from `pop templates`; `--from` also takes a template's path, such as one pop build SOURCE wrote. Never edit a master under ~/.pop/templates or copy one by hand. Open it with `pop open PATH`.

When creating source on the user's behalf, use `pop init SOURCE --yes --brief
'What the pop should do'`, then set the title, description, and
categories in pop.ts to match what you build (init flags can also set them). Read the generated BRIEF.md and AGENTS.md before building.
Human interactive setup can instead launch the user's chosen agent CLI after
scaffolding. Do not start a nested agent when you are already implementing it.

Run pop inspect DOCUMENT, then pop describe DOCUMENT: it lists the fields, current values and the app's named commands. When a command matches the task, prefer `pop call DOCUMENT NAME --args JSON`: it runs the app's own action as one atomic, undoable edit, and a refusal (reason `refused`) changes nothing. Otherwise apply typed operations through pop apply/batch, after pop schema and pop get; never open a .pop file with SQLite or keep a JSON copy of a document. Open-document CLI routes to its native owner; closed editing uses the same engine. After an uncertain mutation, run get before another edit. Never automatically replay mutations.

Use File → Export PNG/PDF or pop export DOCUMENT --format png|pdf --output FILE to deliver output. Exports render saved data through the optional Export.svelte component; a pop without it cannot export PNG/PDF. The export flushes drafts and persistence, copies saved state, and renders independently. PNG and selectable vector PDF use Takumi in the restricted Rust renderer.
