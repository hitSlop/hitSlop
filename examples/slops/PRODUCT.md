# hitSlop examples

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Everyday people using a small personal tool alongside their other work. A first
opening should require no onboarding or knowledge of hitSlop's implementation.

## Product Purpose

Each slop does one understandable job. Success means a person can identify the
job, read the current state, and take the next action within seconds. A slop
should have enough character that someone wants to keep it on their desktop.

## Operating Context

Slops are portable local documents hosted in native windows, with disposable
browser previews for development. People can open, duplicate, share, and keep
them. Loro owns live structured state; the host persists its bytes. Each document owns its UI.
The supported authored examples use Svelte, Bits UI, the document DSL, and theme colors declared in `slop.ts`.

## Capabilities and Constraints

- Each `slop.ts` defines the initial viewport and window presentation.
- Preserve saved data and useful behavior when redesigning an example.
- Each object may have a distinct palette, typography, layout, and material.
- Paper, Instrument, and Skin are optional directions, not mandatory looks.
- Reference images under `_vibe/` are inspiration only and never shipping assets.
- Export and icon views should remain recognizable as the same object.

## Brand Commitments

Unique, expressive, and instantly graspable for the slop's single purpose.
Physical materials and retro software references are welcome when they help
that purpose. They are not required across the collection.

## Product Principles

1. Give each object one clear job and one dominant action or readout.
2. Keep interaction behavior familiar even when the visual identity is unusual.
3. Make state readable without relying on color or metaphor alone.
4. Let each object's purpose determine its expression.
5. Use one working pilot and human feedback to develop the shared guide.

## Accessibility & Inclusion

Readable labels, keyboard operation, visible focus, sufficient contrast, and
reduced-motion behavior are the baseline. Compact windows must not force tiny
essential text or controls.

The host disables text selection on ordinary UI by default. Keep editing and selection
familiar in text fields, and opt useful copyable content into selection with authored
CSS. A whole slop may opt in when reading and copying are central to its purpose.

## Evidence on Hand

Quick Checklist, Hourglass, and Little Lamp are current examples; additional templates
are welcome. Shape Lab lives in `tests/apps/shape-lab` as a presentation fixture.
Each example's visual identity belongs to that object alone.
Use current working examples and local `_vibe/` references as evidence for
clarity and expressiveness, not as a collection-wide skin.
