# hitSlop documentation

hitSlop ships a macOS app and a matching Bun authoring CLI/SDK. Documents stay local; the catalog combines bundled templates, installed templates, and Recents.

| Task | Guide |
| --- | --- |
| Understand how the system works | [Architecture](architecture.md) |
| Scalar kinds and four restored slops (done) | [Scalars plan](ScalarsPlan.md) |
| The current milestone: records, scalar lists and optional text | [Collections plan](CollectionsPlan.md) |
| Rules for platform changes | [Engineering contract](engineering-contract.md) |
| Build or refine a mini app | [Authoring](guides/authoring.md) |
| Run common CLI workflows or look up document operations | [CLI workflows and reference](guides/cli.md) |
| Work on the repository or add templates | [Development](guides/development.md) |
| Choose checks and write tests | [Testing](testing.md) |
| Validate and release the app and npm packages | [Releasing](guides/releasing.md) |
| Package, page shell, storage and security boundaries | [Runtime reference](reference/runtime.md) |
| Deferred capabilities | [Roadmap](roadmap.md) |

The [public tutorial](../apps/landing/src/content/docs/docs/getting-started.mdx) is for authors using the distributed tools. Packaged agent guidance lives in [packages/cli/skills](../packages/cli/skills).

Measurements live in [`evidence/`](evidence/). Superseded plans, the old test ledger, historical benchmarks and the Rust-core spikes are kept in [`archive/`](../archive/) for provenance only; they are not contracts.
