---
name: hitslop
description: Create, fill, open, and export useful hitSlop documents, or build a new mini app when no template fits.
---

# Make a local hitSlop document

Do not display “Saved,” “Saving…,” or routine persistence indicators inside authored slops. The native host owns save-failure and retry UI. Use task-specific feedback for explicit operations, such as “Importing skin…” or “Skin applied.”

Use `bunx @hitslop/cli`, or install with `bun install -g @hitslop/cli` and use `slop`. Installed macOS editing uses the app's bundled slop-engine without Node/Bun. Use the Mac catalog browser for templates registered under ~/.hitslop/templates. File → New from Template opens that browser. For new source read hitslop-authoring and hitslop-design. Local templates and recents work offline. Remote catalog loading, publication, and sharing are deferred.

Build with slop build SOURCE. Make a writable document with `slop create --from TEMPLATE --output PATH` at a fresh user-selected path; never edit a master under ~/.hitslop/templates or copy one by hand. Open it with `slop open PATH`.

When creating source on the user's behalf, use `slop init SOURCE --yes --brief
'What the slop should do'`, then set the title, description, and
categories in slop.ts to match what you build (init flags can also set them). Read the generated BRIEF.md and AGENTS.md before building.
Human interactive setup can instead launch the user's chosen agent CLI after
scaffolding. Do not start a nested agent when you are already implementing it.

Run slop inspect DOCUMENT, then slop schema DOCUMENT and slop get DOCUMENT. Apply typed operations through slop apply/batch; never open a .slop file with SQLite or keep a JSON copy of a document. Open-document CLI routes to its native owner; closed editing uses the same engine. After an uncertain mutation, run get before another edit. Never automatically replay mutations.

Use File → Export PNG/PDF or slop export DOCUMENT --format png|pdf --output FILE to deliver output. Exports render saved data in a fresh page using the default view. HITSLOP_NATIVE_CLI selects an explicit helper and never silently falls back. The export flushes drafts and persistence, copies saved state, and waits for content in an independent renderer.
