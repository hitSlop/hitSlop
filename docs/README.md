# hitSlop documentation

hitSlop ships a macOS app and a matching Bun authoring CLI and SDK. Documents stay local;
the catalog combines bundled templates, installed templates and Recents.

Authors, including us, start with the
[public guides](../apps/landing/src/content/docs/docs/getting-started.mdx), which are also
published on hitslop.com. The pages here cover how the platform works and how to change it.

| Task | Page |
| --- | --- |
| Where hitSlop is going, and what's open now | [Direction](roadmap.md) |
| Proposals and the reasoning behind them | [Ideas](ideas.md) |
| Understand how an edit, a save and a close move | [Architecture](architecture.md) |
| Rules for platform changes | [Engineering contract](engineering-contract.md) |
| Every document kind: snapshot, merge, writes, handles and CLI paths | [Document types](reference/document-types.md) |
| File layout, limits, security, capture and telemetry | [Runtime reference](reference/runtime.md) |
| Operation shapes, ownership and tool identity | [CLI reference](guides/cli.md) |
| Standards for the templates in this repository | [Authoring templates](guides/authoring.md) |
| Work on the repository or add templates | [Development](guides/development.md) |
| Choose checks and write tests | [Testing](testing.md) |
| Validate and release the app and npm packages | [Releasing](guides/releasing.md) |

Packaged agent guidance lives in [packages/cli/skills](../packages/cli/skills).
Measurements live in [`evidence/`](evidence/). [`archive/`](../archive/) holds historical
material for provenance only; it is not a contract. The executed plans in `archive/docs`
and the spikes in `archive/spikes` are tracked; the rest stays local.
