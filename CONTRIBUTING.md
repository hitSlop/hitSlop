# Contributing to hitSlop

Use the Bun version pinned in root `package.json`. Read [AGENTS](AGENTS.md) and [development](docs/guides/development.md), then install the root and landing dependencies with their frozen lockfiles. Apple work requires Xcode and XcodeGen on macOS.

Keep changes focused and preserve work already in the checkout. Add meaningful tests or a reproducible manual check. For templates, read `slop.ts` first, follow [authoring](docs/guides/authoring.md), and use the discovery/bundled-selection workflow in the development guide.

Run `bun run test:local` before release or a release-ready pull request. Focused commands are documented in the development guide. After changing TypeBox contracts, run `bun run schema:generate` and inspect generated output. Never edit generated contracts or historical compatibility fixtures by hand.

Explain the problem, resulting behavior, and verification in pull requests. Include screenshots for visual changes and call out runtime/package/storage compatibility impact. Keep secrets, signing keys, local documents, dependencies, and generated artifacts out of Git. See [releasing](docs/guides/releasing.md) for publication and manual acceptance.
