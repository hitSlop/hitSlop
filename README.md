# hitSlop

**Tiny apps. Big personality.**

Make little apps for your Mac with Svelte & your AI coding agent: a planner that fits your week, a recipe card covered in your notes, a tiny pond to stare at between meetings.

Each one is a document you and your agent share. Check something off and your agent can read it. Ask your agent to add a row and it appears in your open window. Everything stays on your Mac, in one `.slop` file (a SQLite database) you can keep, copy and send to a friend.

[Download for Mac](https://github.com/hitslop/hitslop/releases/latest/download/hitSlop.dmg) · [Explore the website](https://hitslop.com) · [Make your first slop](#make-your-own-with-an-agent) · [Docs](docs/README.md)

macOS 15.2+ · Apple silicon · No account required

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

The [Tiny Wins tutorial below](#a-whole-slop-from-scratch) defines `countOne`. Each app exposes its own actions through `describe`; `apply` and `batch` also let agents edit individual fields directly. With the window open, each increment appears immediately and the theme command turns the accent purple. **Edit ▸ Undo** can undo agent edits too. Text edits supplied with the version the agent read merge with what you typed since. hitSlop doesn't upload your documents: the agent uses the same Rust document owner as the app, including for closed files.

## Small enough to be yours

Sometimes you just want a packing list for one trip. A timer that looks like a tomato. A recipe card covered in your own notes. Those are good reasons to make software.

hitSlop is built for tools with one clear job and a little character. It comes with a checklist and an hourglass timer to open and make your own. Or ask your agent for the thing you keep wishing existed. It can feel like a sheet of paper, a pocket calculator, or something you found in an old arcade.

## Keep the app. Keep the work.

- Your documents live on your Mac. You don't need an account or a server.
- A `.slop` is one SQLite file: the interface, saved data and imported files travel together. Any SQLite tool can open it to look inside; edit it through hitSlop or the CLI. Close a document before moving it in Finder, and send the file itself to a friend who has hitSlop. [How to share a slop](apps/landing/src/content/docs/docs/guides/build-and-share.mdx#share-a-template-or-a-document).
- Export a PNG or PDF to send an invoice, print a recipe, or drop a plan into a message.
- Use **Share a Copy** to send a saved copy, with its attachments and colors, while you keep working in the original.
- Pin a slop **Always on Top**, or enter fullscreen in apps that enable it, including Checklist and Hourglass.
- Finder icons can show what's inside, such as a counter's total.
- Change a document's colors without touching its code, or edit the source to make a different tool.

## Make your own with an agent

Use Bun 1.4.2 or newer on macOS or Linux. Install the hitSlop Mac app to open windows, register templates and export PNG/PDF.

```sh
bunx hitslop@1.0.0 init weekend-kit
cd weekend-kit
bun install
```

Setup asks what your slop should do, then offers to launch your agent: Codex, Claude Code, Gemini CLI, OpenCode, or another CLI. The starter includes a working checklist, your brief, and hitSlop's guidance for agents. Or open the folder in your agent yourself and try:

> Read AGENTS.md, slop.ts, and the authoring/design skills in .agents/skills first. Turn this starter into "Weekend Kit," a packing list for short trips. Let me add items, group them by bag, check them off, and see how many are left. Make it feel like a pocket field notebook: warm paper, forest-green ink, and comfortable checkboxes. Include a clean printable packing list for PNG/PDF export. Keep it small, use hitSlop's document APIs for saved data, and run the project checks when you're done.

Ask for changes as you go ("Make the checkboxes bigger"). When it feels right:

```sh
bun run dev       # Try it in your browser; preview data resets on refresh
bun run build     # Package it as dist/weekend-kit.slop
bun run register  # Add it to hitSlop's templates
```

Stop the preview before building.

Open hitSlop, choose Weekend Kit under **Templates**, and select **Create**. Existing documents keep the app version they were made with, so make another version whenever you like.

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

Document creation and editing run on macOS and Linux. The CLI carries its own engine; opening windows and exporting PNG/PDF use the Mac app's rendering helper. If you're unsure whether an edit happened, check with `get` before trying again. [CLI workflows](apps/landing/src/content/docs/docs/guides/cli-workflows.mdx) has complete examples.

## Where it's going

Next: ask your agent from the toolbar, see who changed what with a selective “Undo that,” and remix a slop someone sent you. See [direction](docs/roadmap.md) and [ideas](docs/ideas.md).

## Why Svelte?

Svelte keeps a component's markup, behavior, and [scoped CSS](https://svelte.dev/docs/svelte/scoped-styles) together in a `.svelte` file. You can design a paper planner, a calculator, or a pond without adopting a prescribed set of UI components. TypeScript and `bun run check` catch type and template mistakes.

[Svelte's compiler](https://svelte.dev/docs/svelte/svelte-compiler) turns components into JavaScript. `bun run build` bundles that code, the Svelte runtime code it needs, and the app's styles into the `.slop`. The interface keeps the Svelte version it was built with. The Mac app supplies the hitSlop document engine, the page shell and native services; it doesn't supply Svelte.

hitSlop's Svelte bindings connect document snapshots to Svelte 5's [reactive updates](https://svelte.dev/docs/svelte/lifecycle-hooks).

Separate `App.svelte`, `Export.svelte`, and `Icon.svelte` components read the same document. hitSlop mounts capture views only when needed and handles saving and capture. The Tiny Wins example below shows how this works.

Svelte is the supported authoring integration; other frameworks would need an adapter to the document engine.

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

`schema.ts` defines the saved fields. Text is editable; a counter supports increments.

```ts
import { defineDocument, s } from "hitslop";

export default defineDocument({
  title: s.text(),
  wins: s.counter(),
});
```

### 2. Give it a name, a window and colors

`slop.ts` describes the app: its name, its starting window size, the colors people can change, and the starting values for **new** documents. The explicit `slug` identifies the app.

```ts
import { defineSlop } from "hitslop";
import schema from "./schema";
import App from "./App.svelte";
import Export from "./Export.svelte";
import Icon from "./Icon.svelte";
import * as commands from "./commands";
import "./styles.css";

export default defineSlop({
  slug: "tiny-wins",
  view: App,
  export: Export,
  icon: Icon,
  commands,
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
  document: schema,
  initial: { title: "Tiny wins today", wins: 0 },
});
```

Theme colors are CSS variables shared by the window, icon, and export. Change them through the theme panel or CLI, or share them as a theme file, without rebuilding. Changes stay with that document; the template keeps its defaults. Changing `initial` later doesn't overwrite saved wins.

`fullscreenable: true` enables the Mac window's fullscreen control and **View ▸ Enter Full Screen**. See [window behavior](apps/landing/src/content/docs/docs/guides/manifest-and-windows.mdx#enable-fullscreen) for other window types.

### 3. Give the button and CLI the same action

Replace the starter's `commands.ts` with:

```ts
import doc from "./schema";

export const countOne = doc.command({
  description: "Give yourself credit for one little win.",
  args: {},
  run({ tx }) { tx.fields.wins.increment(); },
});
```

Declare commands at module scope and register their exports with `commands` in `defineSlop`, as above. The button imports `countOne`; an agent discovers it with `slop describe` and runs it with `slop call`. Both use the same action, applied atomically as one undo step. Commands with arguments declare them using `s.*` descriptors. Ordinary field handles remain useful for typing and other direct edits.

### 4. Build the app, icon, and export together

`App.svelte` is the whole interface. Import the document from `schema.ts`, read from `doc.current`, write through `doc.fields`, and let hitSlop handle saving.

```svelte
<script lang="ts">
  import { bindText } from "hitslop/svelte";
  import doc from "./schema";
  import { countOne } from "./commands";
</script>
<main class="wins-card">
  <input aria-label="Counter title" use:bindText={doc.fields.title} />
  <p class="wins-number" aria-live="polite">{doc.current.wins}</p>
  <button onclick={() => countOne()}>A little win +1</button>
</main>
```

`Export.svelte`:

```svelte
<script lang="ts">
  import doc from "./schema";
</script>
<article class="wins-card">
  <h1>{doc.current.title}</h1>
  <p class="wins-number">{doc.current.wins}</p>
  <p>Little things add up.</p>
</article>
```

`Icon.svelte`:

```svelte
<script lang="ts">
  import doc from "./schema";
</script>
<div class="wins-icon">{doc.current.wins}</div>
```

`view: App` selects the editor. The declaration also selects two optional capture components:

- `Export.svelte` supplies the layout for previews and PNG/PDF exports. Use normal document flow so long content can expand. Without this component, hitSlop renders a fresh App from saved data with its default local view; mark controls with `data-slop-export="hide"` to leave them out.
- `Icon.svelte` supplies the document's dynamic Finder icon, centered on a transparent 512 × 512 canvas. Without it, Finder can use the saved preview or the generic document icon.

hitSlop refreshes the Finder preview and icon when you close the document. Register renders the template's initial artwork from starting values.

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

The builder connects the explicitly imported views and styles to the host through
`defineSlop`. Filenames do not assign roles. See the
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

A slop is a SQLite file holding a Svelte app and a [Loro](https://loro.dev/) document. One Rust core owns the document, the file and every edit, so the Mac app and the CLI change a slop the same way.

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
share        room, endpoint                                             empty unless the document is shared (coming soon)
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
bun slop dev examples/slops/quick-checklist
```

[Development](docs/guides/development.md) covers setup and adding templates, and the [documentation index](docs/README.md) covers how the platform works.

## Related projects

Other projects exploring personal software and interactive documents:

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
