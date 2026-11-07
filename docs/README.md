# pop documentation

pop ships a macOS app and a matching Bun authoring CLI and SDK. The iPhone app
(iOS 26+) is in TestFlight, with opening, editing, creation, Files import and
Home Screen widgets that refresh in the background when iOS allows. The Mac app
includes the Synced Library.
The [Synced Library and live sharing](architecture.md#sync) extend local
documents across a person's devices and between people; [direction](roadmap.md) tracks rollout.
The Mac catalog combines bundled templates, installed templates and Recents.

Authors, including us, start with the
[public guides](../apps/landing/src/content/docs/docs/getting-started.mdx), which are also
published on dotpop.click. The pages here cover how the platform works and how to change it.

| Task | Page |
| --- | --- |
| Where pop is going, and what's open now | [Direction](roadmap.md) |
| Cloud documents: identity, authority, sync and operations | [Cloud documents](cloud-documents.md) |
| Apple catalog: Inbox, Recents and document badges | [Apple catalog](apple-catalog.md) |
| Proposals and the reasoning behind them | [Ideas](ideas.md) |
| Understand how an edit, a save and a close move | [Architecture](architecture.md) |
| Rules for platform changes | [Engineering contract](engineering-contract.md) |
| Every document kind: snapshot, merge, writes, handles and CLI paths | [Document types](reference/document-types.md) |
| File layout, limits, security, capture and telemetry | [Runtime reference](reference/runtime.md) |
| Icon, preview and export rendering; Mac and iPhone widget publication and refresh | [Capture architecture](architecture.md#close-export-and-capture) and [macOS widgets](architecture.md#macos-image-widgets) |
| Camera, microphone, speech and their permission boundaries | [Capabilities](reference/runtime.md#security-boundaries) |
| Operation shapes, ownership and tool identity | [CLI reference](guides/cli.md) |
| Standards for the templates in this repository | [Authoring templates](guides/authoring.md) |
| Work on the repository or add templates | [Development](guides/development.md) |
| Choose checks and write tests | [Testing](testing.md) |
| Validate and release the app and npm package | [Releasing](guides/releasing.md) |

Packaged agent guidance lives in [packages/pop/skills](../packages/pop/skills).
Measurements live in [`evidence/`](evidence/). They are frozen snapshots of the
builds and environments named in each report, not current release acceptance.
Superseded measurements, historical studies, spikes and working plans stay in the
local, untracked `archive/` directory and are not contracts; the architecture,
direction and ideas above are the shared sources of truth.
