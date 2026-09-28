# Preserved runtime fixtures

These are release baselines, not generated examples. Never rebuild or replace them
when the SDK changes; add new fixtures for new capabilities and contracts.
`fixture.json` seals each `document/` tree; checks reject byte changes, and the history
guard rejects any change to any file of a recorded fixture against the base commit.

Contract 2 / revision 1 (the launch baseline), authored once by
`bun scripts/v1/author-fixtures.ts` over the shared schema in `tests/abi/fixture-schema.ts`:

| Fixture | Covers |
|---|---|
| `2-1` | Every value kind, a checkpoint plus uncheckpointed updates, a scripted `scenario.json` with independently modeled results, and `collaboration.json`: a concurrent offline round (deterministic merges only) and synced edits for mixed-version peers, with an independently modeled `expected` result. Its app is the hand-written plain-JS ABI consumer (`tests/abi/plain/app.js`). |
| `2-1-saved-state` | Theme overrides, a content-addressed attachment referenced from a field, several separately saved updates. |
| `2-1-issues` | Merged anomalies (non-finite counter, wrong container kind, unknown enum, unknown field, missing and duplicate row IDs, which read derived `x-` IDs) with exact `issues.json`; nothing is repaired. |
| `2-1-svelte` | The Svelte adapter as compiled into apps (`tests/abi/svelte`). |

Both consumer apps check the `ctx` ABI while mounting; a failed check fails the native
open or export. `template-<slug>-<hash>` specimens are added by
`bun run fixtures:seal --write` when a release seals its runtime.

Swift tests always edit disposable copies. Host-behavior probes swap in the unsealed
`tests/abi/probe/app.js`.
