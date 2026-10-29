# tinys

**Small apps you can keep as files.**

Make a planner for your week, a little instrument, a recipe book, or a tool for something only you do. A `.tiny` file carries the app, its saved work, and its attachments together. Open it in tinys, make it yours, and share a copy.

You can start with a template or build something with Svelte, TypeScript, and your coding agent. You and your agent edit the same document through the app and the `tiny` CLI.

[Website](https://tinys.click) · [Documentation](docs/README.md) · [Releases](https://github.com/hitSlop/tinys/releases) · [iPhone testing channel][ios-download] · [Community](https://discord.gg/cqKRZjAWv3)

[ios-download]: https://testflight.apple.com/join/F4qfCkgg

## Release status

This repository is preparing **tinys 1.0.0**, a fresh release lineage from hitSlop. The first tinys app downloads and the `@hitslop/tinys` npm publication are pending. The renamed landing page lives in [`apps/landing`](apps/landing) and targets **tinys.click**; production deployment remains gated during the transition.

Documents use **`.tiny`**. Legacy `.slop` documents and authoring APIs are unsupported. Existing hitSlop downloads and TestFlight builds belong to the previous lineage; new tinys builds retain the existing Apple app identity and testing channel.

See the [transition checklist](docs/guides/tinys-transition.md) for the remaining launch setup.

## What a tiny can do

- **Keep your work locally.** Use a document without an account or server. The app and its data live together in one SQLite file.
- **Start from a template.** Create a writable copy of a planner, notebook, game, instrument, or desktop companion. Templates stay unchanged.
- **Work with your agent.** Inspect fields, call the app's named commands, edit data, and change colors through the CLI—even while the document is open. Undo includes agent edits.
- **Share useful things.** Send a `.tiny` file with its attachments, or export PNG and PDF from apps with an export view.
- **Use Mac and iPhone.** The clients support an iCloud Synced Library, responsive interfaces, and widgets. Mac requires macOS 15.2+ on Apple silicon; iPhone requires iOS 26+.
- **Edit together.** Live collaboration supports Mac, iPhone, and Chrome browser copies. The owner signs in with Apple; guests join through a link. Ending a session leaves participants with their own copies.

These capabilities are implemented in the source. The new public domains and release artifacts still need the launch steps above.

## Try the source

Use the Bun version pinned in `package.json`, Rust/Cargo, and the tools in the [development guide](docs/guides/development.md). Native Apple builds also need Xcode and XcodeGen.

```sh
git clone https://github.com/hitSlop/tinys.git
cd tinys
bun install --frozen-lockfile
bun run build
bun run tiny dev tinys/quick-checklist
```

To build the Mac app:

```sh
bun run apple:build
```

The [development guide](docs/guides/development.md) covers native builds, template rebuilds, and iPhone preparation.

## Make a tiny

The combined SDK and CLI package is **`@hitslop/tinys`**. Its terminal command is **`tiny`**.

After the first npm release is published:

```sh
bunx @hitslop/tinys@1.0.0 init weekend-kit
cd weekend-kit
bun install
bun run dev
```

Setup asks what your app should do and can launch your preferred coding agent. The project includes a brief and authoring guides. Metadata goes in `tiny.ts`; `schema.ts` declares the saved data; `App.svelte` supplies the editor. `Preview.svelte` and `Icon.svelte` provide artwork, with optional export and widget views.

Use the project's `bun run check`, `bun run build`, and `bun run register` scripts. Build produces `dist/SLUG.tiny`; register adds the immutable template to the native catalog. Create a writable document from the template before editing it.

For a global CLI installation, after publication:

```sh
bun install -g @hitslop/tinys@1.0.0
tiny templates
tiny create --from quick-checklist --output Checklist.tiny
tiny open Checklist.tiny
tiny describe Checklist.tiny
tiny get Checklist.tiny
```

See [authoring](docs/guides/authoring.md), the [CLI reference](docs/guides/cli.md), and [document types](docs/reference/document-types.md) for the complete workflow.

## Landing page

The Astro website and Starlight docs are in [`apps/landing`](apps/landing). They have their own dependency install and lockfile:

```sh
bun install --cwd apps/landing --frozen-lockfile
bun run --cwd apps/landing dev
bun run landing:check
bun run landing:build
```

The website targets `tinys.click`; sharing targets `share.tinys.click`. Browser mini apps run on isolated `dotslop.com` subdomains. See the [landing setup guide](apps/landing/README.md) before deploying.

## Contributing

Read [AGENTS.md](AGENTS.md), [architecture](docs/architecture.md), and the [engineering contract](docs/engineering-contract.md). Rust owns document semantics and storage; Swift hosts the native clients; the SDK and CLI share one npm package.

Run the repository's verification runner:

```sh
bun run verify
bun run verify --native
```

Tests belong at the shared boundary they protect. Individual templates use existing checks and hands-on preview/export review. See [testing](docs/testing.md).

The [roadmap](docs/roadmap.md) tracks current qualification and future work, including asking an agent from the window, remixing, and more native platforms.

MIT licensed. See [LICENSE](LICENSE).
