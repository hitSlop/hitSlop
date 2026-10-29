# @hitslop/tinys

The tinys author SDK and the `tiny` CLI, shipped together.

Create an app with Bun 1.4.2 or newer:

```sh
bunx @hitslop/tinys@1.0.0 init my-tiny
cd my-tiny
bun install
bun run dev
```

In a terminal, `init` asks what your tiny should do and who the author is, then
offers to launch your preferred agent CLI to build it. The agent sets the title,
description, and categories in `tiny.ts` to match; edit them there anytime. Choose a detected agent, **Other CLI…** for an installed executable
such as Grok, or **Finish without launching**. The project includes `BRIEF.md`,
`AGENTS.md`, and local authoring/design guides. Launch failure keeps the project.

For scripts or an agent already working on your behalf:

```sh
bunx @hitslop/tinys init budget-book --yes \
  --brief 'Track spending by category with a monthly summary.' \
  --title 'Budget Book' \
  --category finance --category personal --author Jordan \
  --description 'A simple monthly spending tracker.'
```

`--yes`, CI, and non-TTY runs never prompt or launch agents. Missing metadata
defaults to the directory name, `productivity`, `Anonymous`, and `A tinys mini app.`.

`init`, `check`, `dev`, `build` and `register` run on macOS or Linux. On both platforms, `build` and `register` also render the preview and icon artwork a project does not supply, and `export` renders a closed document's PNG or PDF, without the Mac app. `open` needs the tinys Mac app (Apple silicon, macOS 15.2+). Hosted template publication is not supported.

## Common workflows

In a generated project, use `bun run check`, `bun run dev`, `bun run build`, and `bun run register`. Build creates `dist/SLUG.tiny`; register adds an immutable template to the Mac app's catalog. Choose **Create** in the app, or run `tiny create --from SLUG --output My.tiny`, to make a writable document before editing; `tiny open My.tiny` opens it. `tiny templates` lists the catalog's templates and their slugs; `--from` also takes a built file such as `dist/SLUG.tiny`.

For an existing writable document:

```sh
bunx @hitslop/tinys@1.0.0 describe My.tiny
bunx @hitslop/tinys@1.0.0 get My.tiny
bunx @hitslop/tinys@1.0.0 theme get My.tiny
bunx @hitslop/tinys@1.0.0 export My.tiny --format pdf --output My.pdf
```

Use `apply` or `batch` for schema-aware edits and `attachments` for portable files. Agent skills link to the global CLI so they update with it: run `bun install -g @hitslop/tinys@1.0.0`, then `tiny skills install` to choose skills and agent targets. Installation is additive and global by default (`--scope project` links one project); `skills repair` fixes broken links and `skills uninstall` removes them. Bare `skills` means install. Project links from `init` and `--scope project` follow the pinned node_modules/@hitslop/tinys install. Add `--help` to inspect a command's arguments.

Alternatively, `bun install -g @hitslop/tinys@1.0.0` provides `tiny` on Bun's PATH. `tiny` needs Bun; the package contains the document engine for its supported platforms.

Follow the [CLI workflows](https://tinys.click/docs/guides/cli-workflows/) for copyable examples, themes, attachments, exports, and skills. The [repository CLI reference](https://github.com/hitSlop/tinys/blob/master/docs/guides/cli.md) includes all document operation shapes and contributor setup.

See the [author guides](https://tinys.click/docs/getting-started/) and [release guide](https://github.com/hitSlop/tinys/blob/master/docs/guides/releasing.md).

The CLI, SDK, templates and skills ship in this one package. MIT licensed.
