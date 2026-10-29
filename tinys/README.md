# Active examples

Every immediate folder here with a `tiny.ts` is an active template; its folder name is its slug. `bundled.json` chooses which templates ship inside the Mac app; a template can be developed here before it is added to that list. The other examples wait in `examples/archive` until they move to `defineTiny`.

```sh
bun tiny dev tinys/quick-checklist
```

Each project uses fixed root roles: `tiny.ts` for metadata, `schema.ts` for fields and
initial values, and `App.svelte` for the editor. `Export.svelte`, `Icon.svelte` and
agent-only `commands.ts` are optional. Shared helpers live in separate modules.
