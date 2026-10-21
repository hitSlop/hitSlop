# hitSlop

**Tiny apps. Big personality.**

[![CI](https://img.shields.io/github/actions/workflow/status/hitSlop/hitSlop/ci.yml?branch=master&style=flat&label=CI)](https://github.com/hitSlop/hitSlop/actions/workflows/ci.yml)
[![MIT License](https://img.shields.io/badge/license-MIT-7050ad?style=flat)](LICENSE)
[![macOS 15.2+](https://img.shields.io/badge/macOS-15.2%2B-555555?style=flat&logo=apple&logoColor=white)](https://hitslop.com)
[![Apple silicon](https://img.shields.io/badge/Apple-silicon-555555?style=flat&logo=apple&logoColor=white)](https://hitslop.com)
[![Discord — show us your slop](https://img.shields.io/badge/Discord-show_us_your_slop-5865F2?style=flat&logo=discord&logoColor=white)](https://discord.gg/cqKRZjAWv3)
[![GitHub stars](https://img.shields.io/github/stars/hitSlop/hitSlop?style=flat&color=e6a23c)](https://github.com/hitSlop/hitSlop/stargazers)

[![Local-first](https://img.shields.io/badge/Local--first-487B41?style=flat)](#keep-the-app-keep-the-work)
[![Local use needs no account](https://img.shields.io/badge/Local_use-no_account-487B41?style=flat)](#keep-the-app-keep-the-work)
[![Tiny apps. Big personality.](https://img.shields.io/badge/Tiny_apps-Big_personality-7050ad?style=flat)](#small-enough-to-be-yours)

Make little apps for your Mac with Svelte & your AI coding agent: a planner that fits your week, a recipe card covered in your notes, a tiny pond to stare at between meetings.

**Install hitSlop once. Make and share little apps as files.** Each `.slop` carries its interface, saved work and imported files. Send a fresh template or a filled document to anyone with hitSlop, without packaging and signing a separate Mac app.

You and your agent work on the same document. Check something off and your agent can read it. Ask your agent to add a row and it appears in your open window. Your documents stay yours to keep, copy and share. The iPhone TestFlight preview brings slops to your phone, with Synced Library across Mac and iPhone through your iCloud account.

[Download for Mac](https://github.com/hitslop/hitslop/releases/latest/download/hitSlop.dmg) · [Explore the website](https://hitslop.com) · [Make your first slop](#make-your-own-with-an-agent) · [Docs](docs/README.md)

macOS 15.2+ · Apple silicon · Local documents need no account

iPhone (iOS 26+) is in TestFlight preview; App Store release coming soon. It opens, edits and creates slops, includes a Quick Checklist starter, and imports from Files. [Availability](https://hitslop.com/docs/availability/) covers Synced Library and the live sharing preview. The next Mac release includes Synced Library on CloudKit Production, matching TestFlight/App Store iPhones. Ordinary Mac releases do not include live collaboration.

<p align="center">
  <a href="apps/landing/public/assets/desktop-hero.mp4"><img src="apps/landing/public/assets/desktop-hero-poster.jpg" width="900" alt="A Mac desktop full of open slops: a Winamp-style music player, a desktop pet, a focus timer, flashcards, a doodle board, a koi pond, Wordle, and school planners."></a>
  <br><sub>▶ <a href="apps/landing/public/assets/desktop-hero.mp4">Watch the 30-second tour</a></sub>
</p>

## You and your agent, one document

A slop is a live document with its own small interface. Your agent works on the same document through the hitSlop CLI. Here's a fresh Tiny Wins counter:

```sh
slop describe "My Wins.slop"
```

```text
Tiny Wins: A little credit for the things you get done.
Version: 000000000000000100000011

Fields
  ["title"]: text (set)
  ["wins"]: counter (increment)

Commands (slop call PATH NAME --args JSON)
  countOne — Give yourself credit for one little win.
    args: {"additionalProperties":false,"properties":{},"type":"object"}

Value (rows retain their $id)
{
  "title": "Tiny wins today",
  "wins": 0
}
```

```sh
slop call "My Wins.slop" countOne
slop get "My Wins.slop"
```

```json
{"title": "Tiny wins today", "wins": 1}
```

```sh
slop apply "My Wins.slop" --op '{"type":"increment","path":["wins"],"by":1}'
slop theme set "My Wins.slop" --values '{"accent":"#7050ad"}'
```

The [Tiny Wins tutorial below](#a-whole-slop-from-scratch) defines `countOne`. Each app exposes its own actions through `describe`; `apply` and `batch` also let agents edit individual fields directly. With the window open, each increment appears immediately and the theme command turns the accent purple. **Edit ▸ Undo** can undo agent edits too. Text edits supplied with the version the agent read merge with what you typed since. The agent edits your local document through the hitSlop CLI, including when the file is closed. Sync and live sharing are optional preview features.

## Small enough to be yours

Sometimes you just want a packing list for one trip. A timer that looks like a tomato. A recipe card covered in your own notes. Those are good reasons to make software.

hitSlop is built for tools with one clear job and a little character. Its templates include planners, music tools, photo and voice notebooks, games, and desktop companions. Or ask your agent for the thing you keep wishing existed. It can feel like a sheet of paper, a pocket calculator, or something you found in an old arcade.

Apps can declare camera and microphone capture and transcribe stored recordings on-device
on a supported Mac. [Capabilities and permissions](apps/landing/src/content/docs/docs/guides/capabilities.mdx)
explains the APIs, system consent, platform limits and future directions. Contributors
should follow the [template and app rebuild order](docs/guides/development.md#build-current-templates-with-the-app)
so the catalog uses their latest changes.

## Keep the app. Keep the work.

- Local documents need no account or server. The preview's Synced Library keeps slops in sync between Mac and iPhone through your iCloud account.
- A `.slop` is one SQLite file: the interface, saved data and imported files travel together. Any SQLite tool can open it to look inside; edit it through hitSlop or the CLI. Close a document before moving it in Finder.
- Send a freshly built template so someone can start their own document. They need hitSlop; opening the template asks where to save their writable copy. [How to share a slop](apps/landing/src/content/docs/docs/guides/build-and-share.mdx#share-a-template-or-a-document).
- Export a PNG or PDF to send an invoice, print a recipe, or drop a plan into a message.
- Use **Share a Copy** to send a saved copy, with its attachments and colors, while you keep working in the original.
- Add a widget for a slop with a widget view: on your Mac desktop, or for library slops on the iPhone Home Screen in TestFlight. Tap to open it; iPhone widgets also refresh in the background, when iOS allows.
- In the developer/TestFlight preview, choose **Live collaboration** in the Share popover. The owner signs in with Apple; anyone with the link can edit in hitSlop on Mac/iPhone or in Chrome at share.hitslop.com. Chrome saves a downloadable browser copy. **Stop** ends the link and everyone keeps their copy. [Live sharing](https://hitslop.com/docs/guides/live-sharing/) explains the preview.
- Pin a slop **Always on Top**, or enter fullscreen in apps that enable it, including Checklist and Hourglass.
- Finder icons can show what's inside, such as a counter's total.
- Change a document's colors without touching its code, or edit the source to make a different tool.

## Make your own with an agent

Use Bun 1.4.2 or newer on macOS or Linux. Install the hitSlop Mac app to open windows and pick your registered templates in its catalog; on both platforms the CLI renders template artwork and exports closed documents as PNG/PDF itself.

```sh
bunx hitslop@1.0.0 init weekend-kit
cd weekend-kit
bun install
```

Setup asks what your slop should do, then offers to launch your agent: Codex, Claude Code, Gemini CLI, OpenCode, or another CLI. The starter already works: it includes a checklist, its icon and export views, your brief, and hitSlop's guidance for agents. Try it before changing anything:

```sh
bun run dev       # Open the browser preview; preview data resets on refresh
```

Ask your agent to customize it, or open the folder in your agent yourself and try:

> Read AGENTS.md, slop.ts, and the authoring/design skills in .agents/skills first. Turn this starter into "Weekend Kit," a packing list for short trips. Let me add items, group them by bag, check them off, and see how many are left. Make it feel like a pocket field notebook: warm paper, forest-green ink, and comfortable checkboxes. Include a clean printable packing list for PNG/PDF export. Keep it small, use hitSlop's document APIs for saved data, and run the project checks when you're done.

Ask for changes as you go ("Make the checkboxes bigger") and see them in the preview. When it feels right, stop the preview, then:

```sh
bun run check     # Check types and components
bun run build     # Package it as dist/weekend-kit.slop
bun run register  # Optional: add it to your hitSlop templates
```

Send `dist/weekend-kit.slop` to someone with hitSlop. They can open it and create their own packing list; the template contains no personal document state.

If you registered it, open hitSlop, choose Weekend Kit under **Templates**, and select **Create**. Existing documents keep the app version they were made with, so make another version whenever you like.

[A whole slop, from scratch](#a-whole-slop-from-scratch) below builds a complete app with its export view and Finder icon · [Full tutorial](apps/landing/src/content/docs/docs/getting-started.mdx) · [Edit a document with your agent](apps/landing/src/content/docs/docs/guides/edit-installed-data.mdx)

## Use the CLI

Run commands with `bunx hitslop@1.0.0`, or install it with `bun install -g hitslop@1.0.0` and use `slop`. Generated projects have their own pinned `bun run` scripts.

| Task | Commands |
| --- | --- |
| Create and preview an app | `init SOURCE`, then the project's `bun run check` and `bun run dev` |
| Package and install a template | `bun run build`, `bun run register` |
| Make a writable document | `create --from TEMPLATE --output DOCUMENT`, then `open DOCUMENT` |
| Inspect and edit a writable document | `schema`, `get`, `apply`, `batch` |
| Discover and run an app's actions | `describe DOCUMENT`, then `call DOCUMENT NAME --args JSON` |
| Move data between documents | `get`, then one `batch` of `insert` operations |
| Customize and share colors, manage files | `theme get/set/reset/export/import`, `attachments list/import/export` |
| Export a PNG or PDF | `export DOCUMENT --format FORMAT --output FILE` (`png` or `pdf`) |
| Install agent guidance | `bun install -g hitslop`, then `slop skills install`; skills update with the global CLI |

Document creation and editing run on macOS and Linux. The CLI carries its own engine, which also exports a closed document as PNG/PDF and renders template artwork on both platforms; opening a window needs the Mac app. If you're unsure whether an edit happened, check with `get` before trying again. [CLI workflows](apps/landing/src/content/docs/docs/guides/cli-workflows.mdx) has complete examples.

## Roadmap

Here’s where we’re heading:

- [x] **iPhone preview.** Open, edit and create slops, import from Files, and add Home Screen widgets in TestFlight on iOS 26+.
- [x] **Synced Library preview.** Keep slops in sync between Mac and iPhone through iCloud.
- [x] **Live collaboration preview.** Edit together through a link on Mac, iPhone or Chrome.
- [ ] **Release the previews.** Bring iPhone to the App Store and polish sharing. The next Mac release includes Synced Library on CloudKit Production.
- [ ] **More platforms.** Explore iPadOS, Android, Windows and Linux native apps.
- [ ] **Ask from the window.** Work with your agent beside your document, see what it changed, and undo its edits.
- [ ] **Remix something you love.** Make someone else’s slop your own, and keep your saved data as the app evolves.
- [ ] **A store for tiny apps.** Discover, publish and install community slops through an app-store-style catalog.
- [ ] **More native capabilities.** Calendars, reminders, notifications, Shortcuts and Siri, “Send to slop,” MIDI, and richer audio and transcription—with permissions for each slop.

See [direction](docs/roadmap.md) for priorities and [ideas](docs/ideas.md) for the proposals. Preview features are labeled above; remaining directions have no release dates yet.

**What would you like to see?** [Tell us on Discord](https://discord.gg/cqKRZjAWv3).

## Why Svelte?

A `.svelte` file looks like a small web page: a `<script>`, some HTML and a `<style>`. That's the shape of a slop. You write `class`, `onclick` and `{doc.current.wins}` in ordinary markup, with no JSX or hooks, and the page updates when the document does.

- Styles are [scoped](https://svelte.dev/docs/svelte/scoped-styles) to their component, so a slop can be all personality in plain CSS.
- Reactivity is part of the language. Read `doc.current` in markup and Svelte keeps it current, with no dependency arrays to get subtly wrong.
- Transitions and [motion](https://svelte.dev/docs/svelte/svelte-motion) are built in, for the hourglass that tips and the koi that drift.
- Mistakes are loud: `bun run check` catches type and template errors before you build.

Svelte is compiled into the `.slop` with the app, so a document keeps the Svelte it was built with; the Mac app supplies the document engine and native services, not Svelte. Svelte is the supported integration today; other frameworks would need an adapter.

## A whole slop, from scratch

**Tiny Wins** is a complete example: a name, a counter, and a button for giving yourself a little credit. Its window, Finder icon, and export all read the same document.

With the same Bun and Mac app setup above, create a fresh starter:

```sh
bunx hitslop@1.0.0 init tiny-wins
cd tiny-wins
bun install
```

Replace the following starter files. Keep the generated `package.json` and `tsconfig.json`.

### 1. Say what it remembers

`schema.ts` defines the saved fields and what a new document starts with. Text is editable; a counter supports increments. It also holds the one action the button and your agent share:

```ts
import { defineDocument, s, type Tx } from "hitslop";

const schema = defineDocument({
  title: s.text(),
  wins: s.counter(),
}, { initial: { title: "Tiny wins today" } });
export default schema;

/** Give yourself credit for one little win. */
export const countOne = (tx: Tx<typeof schema>) => tx.fields.wins.increment();
```

Changing `initial` later doesn't overwrite saved wins.

### 2. Give it a name, a window and colors

`slop.ts` describes the app: its name, its starting window size and the colors people can change. The explicit `slug` identifies the app.

```ts
import { defineSlop } from "hitslop";

export default defineSlop({
  slug: "tiny-wins",
  title: "Tiny Wins",
  description: "A little credit for the things you get done.",
  author: { name: "You" },
  categories: ["personal"],
  window: { kind: "standard", width: 360, height: 360, fullscreenable: true },
  theme: {
    surface: "#fff7e6",
    ink: "#382d24",
    accent: "#28634b",
  },
});
```

Theme colors are CSS variables shared by the window, icon, and export. Change them through the theme panel or CLI, or share them as a theme file, without rebuilding. Changes stay with that document; the template keeps its defaults.

`fullscreenable: true` enables the Mac window's fullscreen control and **View ▸ Enter Full Screen**. See [window behavior](apps/landing/src/content/docs/docs/guides/manifest-and-windows.mdx#enable-fullscreen) for other window types.

### 3. Give the button and CLI the same action

Replace the starter's `commands.ts` with:

```ts
import { defineCommands } from "hitslop/commands";
import schema, { countOne } from "./schema";

export default defineCommands(schema, {
  countOne: {
    description: "Give yourself credit for one little win.",
    args: {},
    run: ({ tx }) => countOne(tx),
  },
});
```

The button runs `countOne` with `doc.change(countOne)`; an agent discovers the command with `slop describe` and runs it with `slop call`. Both apply the same action atomically, as one undo step. Commands are for agents, and their arguments are flat `s.*` scalars. Ordinary field handles remain useful for typing and other direct edits.

### 4. Build the app, icon, and export together

`App.svelte` is the whole interface. Import the definition from `schema.ts` and acquire the document with `getDocument(schema)` while the component initializes. Read `doc.current`, write through `doc.fields`, and let hitSlop handle saving. See [document access](apps/landing/src/content/docs/docs/guides/data-and-schemas.mdx#read-snapshots-write-through-handles) for child components and helpers.

```svelte
<script lang="ts">
  import { bindText, getDocument } from "hitslop/svelte";
  import schema, { countOne } from "./schema";
  import "./styles.css";
  const doc = getDocument(schema);
</script>
<main class="wins-card">
  <input aria-label="Counter title" use:bindText={doc.fields.title} />
  <p class="wins-number" aria-live="polite">{doc.current.wins}</p>
  <button onclick={() => doc.change(countOne)}>A little win +1</button>
</main>
```

`Preview.svelte` reads its saved `snapshot` prop. [Capture props](apps/landing/src/content/docs/docs/guides/icons-and-exports.mdx#declare-capture-views) also supply image URLs and the view's mode:

```svelte
<script lang="ts">
  import type { CaptureMode, CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  let { snapshot }: CaptureProps<typeof schema> & { mode: CaptureMode } = $props();
</script>
<article class="wins-card">
  <h1>{snapshot.title}</h1>
  <p class="wins-number">{snapshot.wins}</p>
  <p>Little things add up.</p>
</article>
```

For this small counter, optional `Export.svelte` can reuse the complete preview:

```svelte
<script lang="ts">
  import type { CaptureMode, CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import Preview from "./Preview.svelte";
  let props: CaptureProps<typeof schema> & { mode: CaptureMode } = $props();
</script>
<Preview {...props} />
```

`Icon.svelte`:

```svelte
<script lang="ts">
  import type { CaptureMode, CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  let { snapshot }: CaptureProps<typeof schema> & { mode: CaptureMode } = $props();
</script>
<div class="wins-icon">{snapshot.wins}</div>
```

`App.svelte` is the editor. Separate components supply saved-state captures:

- Required `Preview.svelte` supplies a compact overview for Quick Look and in-app previews.
- Optional `Export.svelte` supplies complete PNG/PDF output. Use normal document flow so long content can expand. Without it, export actions are hidden.
- Required `Icon.svelte` supplies the document's dynamic Finder icon, centered on a transparent 512 × 512 canvas. Missing or failed artwork can still fall back to the preview or generic document icon.
- `Widget.svelte` supplies small, medium and large widget images from saved data for Mac and, in the TestFlight preview, library slops on iPhone. The phone renders its own images, including in the background when iOS allows. Saves, watched file changes and startup freshness checks refresh them while hitSlop runs; WidgetKit controls when the new image appears.

hitSlop refreshes the Finder preview and icon when you close the document. On macOS and Linux, build and register render the template's initial artwork from starting values.

Capture views use Takumi: restricted rendering on Mac and Linux, and in-process rendering on iPhone. Mac and Linux support selectable vector PDF. Capture components must be synchronous. Quick Look reads the saved artwork.

See [icons, previews, and exports](apps/landing/src/content/docs/docs/guides/icons-and-exports.mdx) for capture details and size limits.

Style the views in `styles.css`:

```css
* { box-sizing: border-box; }
body { font-family: system-ui, sans-serif; color: var(--slop-ink); }
.wins-card {
  padding: 28px;
  text-align: center;
  overflow-wrap: anywhere;
  background: var(--slop-surface);
}
main.wins-card { min-height: 100%; }
.wins-card input {
  width: 100%; padding: 10px; border: 0; border-radius: 8px;
  font: inherit; color: inherit; background: transparent; text-align: center;
}
.wins-card h1 { margin: 0; font-size: 24px; }
.wins-number { margin: 20px 0; font-size: 72px; font-weight: 800; }
.wins-card button {
  min-height: 44px; padding: 12px 20px; border: 0; border-radius: 12px;
  font: inherit; color: var(--slop-surface); background: var(--slop-accent);
  cursor: pointer;
}
.wins-card :focus-visible { outline: 3px solid var(--slop-accent); outline-offset: 4px; }
.wins-icon {
  width: 440px; height: 440px; border-radius: 100px;
  display: grid; place-items: center; font-size: 150px; font-weight: 800;
  color: var(--slop-accent); background: var(--slop-surface);
}
```

The builder finds each part by its filename: `slop.ts`, `schema.ts` and `App.svelte`, plus
required `Preview.svelte` and `Icon.svelte`, optional `Export.svelte` and `commands.ts`. Stylesheets are imported from
components, as `App.svelte` does above. See the
[runtime reference](docs/reference/runtime.md#page-shell-and-ctx) for the app interface.

### 5. Take it for a spin

```sh
bun run check     # Check the types and Svelte component
bun run dev       # Try it in the browser; preview data resets on refresh
```

The preview reloads as you edit. Stop it before continuing.

```sh
bun run build     # Create dist/tiny-wins.slop (works on Linux too)
bun run register  # Render its preview and icon, then add Tiny Wins to your catalog
```

In hitSlop, choose **Tiny Wins → Create**, then save your document as `My Wins.slop`. Change its title, add some wins, export a PNG/PDF, and close and reopen it to see the saved values. Check its refreshed icon in Finder, too.

Try the commands from [You and your agent, one document](#you-and-your-agent-one-document) on your saved document. `slop theme reset "My Wins.slop"` restores the template's colors.

## How it's built

[![Rust](https://img.shields.io/badge/Rust-ce7043?style=flat&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Swift](https://img.shields.io/badge/Swift-F05138?style=flat&logo=swift&logoColor=white)](https://www.swift.org/)
[![Svelte 5](https://img.shields.io/badge/Svelte-5-FF3E00?style=flat&logo=svelte&logoColor=white)](https://svelte.dev/)
[![TypeScript](https://img.shields.io/badge/TypeScript-3178C6?style=flat&logo=typescript&logoColor=white)](https://www.typescriptlang.org/)
[![SQLite](https://img.shields.io/badge/SQLite-003B57?style=flat&logo=sqlite&logoColor=white)](https://www.sqlite.org/)
[![Bun](https://img.shields.io/badge/Bun-282522?style=flat&logo=bun&logoColor=white)](https://bun.sh/)
[![hitslop on npm](https://img.shields.io/badge/npm-hitslop-CB3837?style=flat&logo=npm&logoColor=white)](https://www.npmjs.com/package/hitslop)

A slop is a SQLite file holding a Svelte app and a [Loro](https://loro.dev/) document. Loro's CRDTs (conflict-free replicated data types) merge edits to text, lists and counters. One Rust core owns the document, the file and every edit, so the Mac app and the CLI change a slop the same way.

| Layer | Built with | Role |
| --- | --- | --- |
| Document engine | Rust, Loro | Merging text, lists and counters; atomic edits; the single owner of the document |
| File | SQLite | The `.slop`: app, assets, artwork and saved state in one database |
| Mac app | Swift, SwiftUI, AppKit, WebKit | Windows, catalog, Quick Look and PNG/PDF export; a thin native layer over the Rust core via UniFFI |
| Slop interface | Svelte 5, TypeScript | The authored app, rendered from document snapshots; the page holds no CRDT |
| Author SDK and CLI | `hitslop`, Bun | Schemas, `slop dev` (Vite with a native Rust owner), build, edit and export |
| Contracts | Rust serde, ts-rs, UniFFI | Rust owns internal wires and app acceptance; generated TypeScript and typed Swift |

```text
page (WebKit) ──▶ Swift façade ──▶ Rust owner ──▶ .slop
slop CLI ───────────────────────▶ Rust owner      (the open window's, or the closed file's lock)
```

- App and agent edits take one path, so an open window updates live and a closed file is edited without starting WebKit.
- Text merges character by character.
- A failed save keeps the document open and shows a retry.

### Inside a .slop

```text
app          markers, catalog columns, definition_json               one immutable declaration
assets       key, media_type, encoding, size, bytes                    ui.js, ui.css, commands.js, media
artwork      name (preview | icon), png                                 Quick Look preview and Finder icon
-- populated when you create and edit a document
document     uuid                                                       the document's identity
history      seq, bytes                                                 saved Loro state: a snapshot, then the edits saved since
share        room, endpoint                                             room and endpoint for the live sharing preview; empty when unshared
attachments  id (SHA-256), media_type, bytes                           files you import
```

Any SQLite tool shows these tables. `history` holds Loro bytes rather than rows, so change a slop's data through hitSlop or the CLI, not SQL. All seven tables exist in templates and documents. A template has app assets, optional artwork and a seed Loro snapshot; creating a document copies it and adds its own identity. The file's SQLite `application_id` and `user_version` mark its format, so a newer file is refused rather than rewritten.

[Architecture](docs/architecture.md) · [Engineering contract](docs/engineering-contract.md) · [Compatibility](docs/engineering-contract.md#compatibility)

## Work on hitSlop

Use the Bun version pinned in `package.json`, Xcode, and XcodeGen on macOS:

```sh
bun install --frozen-lockfile
bun install --cwd apps/landing --frozen-lockfile
bun run build
bun run verify --all --native
bun slop dev slops/quick-checklist
```

[Development](docs/guides/development.md) covers setup and adding templates, and the [documentation index](docs/README.md) covers how the platform works.

## Show us your slop

Made something wonderfully specific? Show us your slop. Share a screenshot, swap ideas, or help someone make their first tiny app.

[![Join hitSlop on Discord](https://img.shields.io/badge/Discord-come_hang_out-5865F2?style=flat&logo=discord&logoColor=white)](https://discord.gg/cqKRZjAWv3)
[![Follow hitSlop on X](https://img.shields.io/badge/X-%40hitslop-181717?style=flat&logo=x&logoColor=white)](https://x.com/hitslop)

### Little things add up

[![hitSlop repository activity and contributor insights](https://repobeats.axiom.co/api/embed/7fa6efb86563307a3adfcfd775c1f4ece1e346de.svg "Repobeats analytics image")](https://github.com/hitSlop/hitSlop/graphs/contributors)

### Star history

<a href="https://www.star-history.com/#hitSlop/hitSlop&Date">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=hitSlop/hitSlop&amp;type=Date&amp;theme=dark" />
    <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=hitSlop/hitSlop&amp;type=Date" />
    <img alt="hitSlop GitHub star history chart" src="https://api.star-history.com/svg?repos=hitSlop/hitSlop&amp;type=Date" width="800" />
  </picture>
</a>

## Related projects

Other projects exploring personal software and interactive documents:

- [Your executable is a SQLite database](https://fzakaria.com/2026/08/23/your-executable-is-a-sqlite-database) and [Actually Queryable Executables](https://fzakaria.com/2026/08/24/actually-queryable-executables): Farid Zakaria's SELF, a program and its state in one SQLite file. It inspired the `.slop` file.
- [Hyperclay](https://hyperclay.com/): HTML files you can reshape in place, where the live document is the source of truth.
- [Capsule](https://withcapsule.app/): documents that run like apps, shared as single SQLite files.
- [bento](https://bento.page/): an office suite that fits in one self-contained HTML file.
- [Decker](https://beyondloom.com/decker/): HyperCard-style decks of interactive cards.
- [TiddlyWiki](https://tiddlywiki.com/): a personal wiki that lives in one HTML file.
- [uapp](https://thederf.com/uapp/demo)

Older inspirations: [HyperCard](https://www.computerhistory.org/revolution/the-web/20/373/2081) and [Smalltalk](https://squeak.org/) put making your own tools within reach. [Winamp's skin system](https://skins.webamp.org/) was a major inspiration for hitSlop's look and feel.

## Privacy

Release builds of the Mac app enable Firebase Analytics and Crashlytics. They send
launch, open, create (source), duplicate and export (format) events; operation
breadcrumbs; failure records with fixed operation, classification, reason and format
fields; crash reports; and Firebase's standard app and device data.

Our custom event, breadcrumb and failure fields do not include document contents or
file paths. Automatic crash reports contain diagnostic information collected by the
Firebase SDK. Collection is enabled in release builds with no in-app toggle; Debug
builds and tests do not upload telemetry.

See [Privacy](https://hitslop.com/docs/privacy/) for the public disclosure.

## License

MIT © 2026 hitSlop contributors. See [LICENSE](LICENSE) and [third-party notices](THIRD_PARTY_NOTICES.md).
