---
name: hitslop
description: Create, fill, open, and export useful hitSlop documents, or build a new mini app when no template fits.
---

# Make a local hitSlop document

Do not display “Saved,” “Saving…,” or routine persistence indicators inside authored slops. The native host owns save-failure and retry UI. Use task-specific feedback for explicit operations, such as “Importing skin…” or “Skin applied.”

Use `bunx @hitslop/cli`, or install with `bun install -g @hitslop/cli` and use `slop`; both need Bun. On a Mac, `slop` edits documents through the app's bundled engine. `slop templates` lists the templates the Mac catalog shows as JSON: each one's slug, title, description, categories, source (`bundled` with the app, or `installed` under ~/.hitslop/templates) and path. People browse the same catalog with File → New from Template. Start from a listed template that fits; when none does, read hitslop-authoring and hitslop-design for new source. Local templates and recents work offline. Remote catalog loading, publication, and sharing are deferred.

Make a writable document with `slop create --from SLUG --output PATH` at a fresh user-selected path, where SLUG comes from `slop templates`; `--from` also takes a template's path, such as one slop build SOURCE wrote. Never edit a master under ~/.hitslop/templates or copy one by hand. Open it with `slop open PATH`.

When creating source on the user's behalf, use `slop init SOURCE --yes --brief
'What the slop should do'`, then set the title, description, and
categories in slop.ts to match what you build (init flags can also set them). Read the generated BRIEF.md and AGENTS.md before building.
Human interactive setup can instead launch the user's chosen agent CLI after
scaffolding. Do not start a nested agent when you are already implementing it.

Run slop inspect DOCUMENT, then slop schema DOCUMENT and slop get DOCUMENT. Apply typed operations through slop apply/batch; never open a .slop file with SQLite or keep a JSON copy of a document. Open-document CLI routes to its native owner; closed editing uses the same engine. After an uncertain mutation, run get before another edit. Never automatically replay mutations.

Use File → Export PNG/PDF or slop export DOCUMENT --format png|pdf --output FILE to deliver output. Exports render saved data in a fresh page using the default view. The export flushes drafts and persistence, copies saved state, and waits for content in an independent renderer.
