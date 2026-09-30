# @hitslop/cli

Create an app with Bun 1.4.2 or newer:

```sh
bunx @hitslop/cli@4.0.0 init my-slop
cd my-slop
bun install
bun run dev
```

In a terminal, `init` asks what your slop should do and who the author is, then
offers to launch your preferred agent CLI to build it. The agent sets the title,
description, and categories in `manifest.json` to match; edit them there anytime. Choose a detected agent, **Other CLI…** for an installed executable
such as Grok, or **Finish without launching**. The project includes `BRIEF.md`,
`AGENTS.md`, and local authoring/design guides. Launch failure keeps the project.

For scripts or an agent already working on your behalf:

```sh
bunx @hitslop/cli init budget-book --yes \
  --brief 'Track spending by category with a monthly summary.' \
  --title 'Budget Book' --slug budget-book \
  --category finance --category personal --author Jordan \
  --description 'A simple monthly spending tracker.'
```

`--yes`, CI, and non-TTY runs never prompt or launch agents. Missing metadata
defaults to the directory name, `productivity`, `Anonymous`, and `A hitSlop mini app.`.

Build and register require the hitSlop Mac app (Apple silicon, macOS 15.2+). Installed native document editing requires neither Node nor Bun. Hosted template publication is not supported.

## Common workflows

In a generated project, use `bun run check`, `bun run dev`, `bun run build`, and `bun run register`. Build creates `dist/SLUG.slop`; register adds an immutable template to the Mac app's catalog. Choose **Create** in the app, or run `slop create --from dist/SLUG.slop --output My.slop`, to make a writable document before editing; `slop open My.slop` opens it.

For an existing writable document:

```sh
bunx @hitslop/cli@4.0.0 schema My.slop
bunx @hitslop/cli@4.0.0 get My.slop
bunx @hitslop/cli@4.0.0 theme get My.slop
bunx @hitslop/cli@4.0.0 export My.slop --format pdf --output My.pdf
```

Use `apply` or `batch` for schema-aware edits and `attachments` for portable files. Agent skills link to the global CLI so they update with it: run `bun install -g @hitslop/cli@4.0.0`, then `slop skills install` to choose skills and agent targets. Installation is additive and global by default (`--scope project` links one project), and the global CLI repairs broken global links as it runs; `skills uninstall` removes them. Bare `skills` means install. The portable guides copied by `init` are ordinary files and do not update with the global CLI. Add `--help` to inspect a command's arguments.

Alternatively, `bun install -g @hitslop/cli@4.0.0` provides `slop` on Bun's PATH. Direct native commands use `"/Applications/hitSlop.app/Contents/Helpers/hitslop-native"`.

Follow the [CLI workflows](https://hitslop.com/docs/guides/cli-workflows/) for copyable examples, themes, attachments, exports, and skills. The [repository CLI reference](https://github.com/hitSlop/hitslop/blob/master/docs/guides/cli.md) includes all document operation shapes and contributor setup.

See the [author guides](https://hitslop.com/docs/getting-started/) and [release guide](https://github.com/hitSlop/hitslop/blob/master/docs/guides/releasing.md).

The CLI pins the matching `@hitslop/document` and `@hitslop/schema` versions. MIT licensed.
