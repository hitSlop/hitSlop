# @hitslop/cli

Create an app with Bun 1.4.2 or newer:

```sh
bunx @hitslop/cli@1.2.0 init my-slop
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

Build and register require the compatible hitSlop Mac app (Apple silicon, macOS 14+). Installed native document editing requires neither Node nor Bun. Hosted template publication is not supported.

## Common workflows

In a generated project, use `bun run check`, `bun run dev`, `bun run build`, and `bun run register`. Build creates `dist/SLUG.slop`; register adds an immutable template to the Mac app's catalog. Choose **Create** to make a writable document before editing.

For an existing writable document:

```sh
bunx @hitslop/cli@1.2.0 schema My.slop
bunx @hitslop/cli@1.2.0 get My.slop
bunx @hitslop/cli@1.2.0 theme get My.slop
bunx @hitslop/cli@1.2.0 export My.slop --format pdf --output My.pdf
```

Use `apply` or `batch` for schema-aware edits, `import` for complete JSON data, and `attachments` for portable files. Run `bunx @hitslop/cli@1.2.0 skills install` to choose skills and agent targets. Installation is additive; use `skills repair` to repair links and `skills uninstall` to remove them. Bare `skills` and the old `skills update` spelling remain aliases. The portable guides copied by `init` are ordinary files and are not refreshed by link repair. Add `--help` to inspect a command's arguments.

Alternatively, `bun install -g @hitslop/cli@1.2.0` provides `slop` on Bun's PATH. Direct native commands use `"/Applications/hitSlop.app/Contents/Helpers/hitslop-native"`; `create` and `open` are available only through that helper.

Follow the [CLI workflows](https://hitslop.com/docs/guides/cli-workflows/) for copyable examples, import replacement rules, themes, attachments, exports, and skills. The [repository CLI reference](https://github.com/hitSlop/hitslop/blob/master/docs/guides/cli.md) includes all document operation shapes and contributor setup.

See the [authoring guide](https://github.com/hitSlop/hitslop/blob/master/docs/guides/authoring.md) and [release guide](https://github.com/hitSlop/hitslop/blob/master/docs/guides/releasing.md).

CLI 2.0.0 uses hitSlop SDK 2.0.0, runtime contract 2 / revision 1. MIT licensed.
