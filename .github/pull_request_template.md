## What changed?

<!-- Explain the problem and the outcome. -->

## How was it verified?

<!-- Mark the applicable tiers and include failures or checks not run. See docs/testing.md. -->

- [ ] Everyday: `bun run check` and `bun run test`
- [ ] Native changes: native boundary and full render checks from `docs/testing.md`
- [ ] Release candidate: `bun run release:check` on the final clean commit
- [ ] Visual changes include screenshots
- [ ] Manifest/schema changes include regenerated artifacts
- [ ] No secrets, local documents, or private paths are included
