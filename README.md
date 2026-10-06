# hitSlop

**Tiny apps. Big personality.**

Make little apps for your Mac with Svelte & your AI coding agent: a planner that fits your week, a recipe card covered in your notes, a tiny pond to stare at between meetings.

Each one is a document you and your agent share. Check something off and your agent can read it. Ask your agent to add a row and it appears in your open window. Everything stays on your Mac, in one `.slop` you can keep, copy and send to a friend.

[Download for Mac](https://github.com/hitslop/hitslop/releases/latest/download/hitSlop.dmg) · [Explore the website](https://hitslop.com) · [Make your first slop](#make-your-own-with-an-agent) · [Docs](docs/README.md)

macOS 15.2+ · Apple silicon · No account required

<p align="center">
  <a href="apps/landing/public/assets/desktop-hero.mp4"><img src="apps/landing/public/assets/desktop-hero-poster.jpg" width="900" alt="A Mac desktop full of open slops: a Winamp-style music player, a desktop pet, a focus timer, flashcards, a doodle board, a koi pond, Wordle, and school planners."></a>
  <br><sub>▶ <a href="apps/landing/public/assets/desktop-hero.mp4">Watch the 30-second tour</a></sub>
</p>

## You and your agent, one document

A slop isn't a chat transcript or a web page someone generated for you. It's a live document with its own small interface, and your agent works on the same document you do, through the hitSlop CLI. Here it works on a tiny counter:

```sh
slop get "My Wins.slop"      # read what you've done in the window
slop apply "My Wins.slop" --op '{"type":"increment","path":["wins"],"by":1}'
slop theme set "My Wins.slop" --values '{"accent":"#7050ad"}'
```

With the window open, the count ticks up and the accent turns purple as each command runs. Text merges character by character, so your agent's edit doesn't wipe out what you're typing elsewhere in the same field. hitSlop doesn't upload your documents: the agent edits them through the hitSlop app on your Mac, even while they're closed.

## Small enough to be yours

Sometimes you just want a packing list for one trip. A timer that looks like a tomato. A recipe card covered in your own notes. Those are good reasons to make software.

hitSlop is built for tools with one clear job and a little character. It comes with starter apps to open and make your own: checklists, a kanban board, a habit heatmap, recipe cards, morning pages, a doodle board, an alien radio, Wordle, and more. Or ask your agent for the thing you keep wishing existed. It can feel like a sheet of paper, a pocket calculator, or something you found in an old arcade.

## Keep the app. Keep the work.

- Your documents live on your Mac. You don't need an account or a server.
- The interface, saved data and imported files travel together. Close a document before moving it in Finder, and zip it to send to a friend who has hitSlop. [How to share a slop](apps/landing/src/content/docs/docs/guides/build-and-share.mdx#share-a-template-or-a-document).
- Export a PNG or PDF to send an invoice, print a recipe, or drop a plan into a message.
- Finder icons can show what's inside, such as a counter's total.
- Change a document's colors without touching its code, or edit the source to make a different tool.

## Make your own with an agent

You need Bun 1.4.2 or newer and the hitSlop Mac app.

```sh
bunx @hitslop/cli@4.0.0 init weekend-kit
cd weekend-kit
bun install
```

Setup asks what your slop should do, then offers to launch your agent: Codex, Claude Code, Gemini CLI, OpenCode, or another CLI. The starter includes a working checklist, your brief, and hitSlop's guidance for agents. Or open the folder in your agent yourself and try:

> Read AGENTS.md, manifest.json, and the authoring/design skills in .agents/skills first. Turn this starter into "Weekend Kit," a packing list for short trips. Let me add items, group them by bag, check them off, and see how many are left. Make it feel like a pocket field notebook: warm paper, forest-green ink, and comfortable checkboxes. Include a clean printable packing list for PNG/PDF export. Keep it small, use hitSlop's document APIs for saved data, and run the project checks when you're done.

Ask for changes as you go ("Make the checkboxes bigger"). When it feels right:

```sh
bun run dev       # Try it in your browser; preview data resets on refresh
bun run build     # Package it as dist/weekend-kit.slop
bun run register  # Add it to hitSlop's templates
```

Open hitSlop, choose Weekend Kit under **Templates**, and select **Create**. Existing documents keep the app version they were made with, so make another version whenever you like.

[A whole slop, from scratch](#a-whole-slop-from-scratch) below builds a complete app with its export view and Finder icon · [Full tutorial](apps/landing/src/content/docs/docs/getting-started.mdx) · [Edit a document with your agent](apps/landing/src/content/docs/docs/guides/edit-installed-data.mdx)

## Use the CLI

Run commands with `bunx @hitslop/cli@4.0.0`, or install it with `bun install -g @hitslop/cli@4.0.0` and use `slop`. Generated projects have their own pinned `bun run` scripts.

| Task | Commands |
| --- | --- |
| Create and preview an app | `init SOURCE`, then the project's `bun run check` and `bun run dev` |
| Package and install a template | `bun run build`, `bun run register` |
| Make a writable document | `create --from TEMPLATE --output DOCUMENT`, then `open DOCUMENT` |
| Inspect and edit a writable document | `schema`, `get`, `apply`, `batch` |
| Move data between documents | `get`, then one `batch` of `insert` operations |
| Customize and share colors, manage files | `theme get/set/reset/export/import`, `attachments list/import/export` |
| Export a PNG or PDF | `export DOCUMENT --format FORMAT --output FILE` (`png` or `pdf`) |
| Install agent guidance | `bun install -g @hitslop/cli`, then `slop skills install`; skills update with the global CLI |

Document commands work through the installed Mac app, even while it's closed. Call `"/Applications/hitSlop.app/Contents/Helpers/hitslop-native"` directly to edit without Node or Bun. If you're unsure whether an edit happened, check with `get` before trying again. [CLI workflows](apps/landing/src/content/docs/docs/guides/cli-workflows.mdx) has complete examples.

## Where it's going

Next, we want changing a slop to be as easy as using it. Its colors already change from the window's theme panel; next come asking your agent from the toolbar, seeing who changed what and undoing an agent's change, and remixing a slop someone sent you. See [direction](docs/roadmap.md) and [ideas](docs/ideas.md).

## Why Svelte?

Svelte suits the kinds of apps we want to make: small tools with custom interfaces that are easy to work on.

You can keep a component's markup, behavior, and [scoped CSS](https://svelte.dev/docs/svelte/scoped-styles) together in a `.svelte` file. That makes a small interface easy to follow and gives you room to design a paper planner, a calculator, or a pond without adopting a prescribed set of UI components. TypeScript and `bun run check` help catch type and template mistakes as you work.

[Svelte's compiler](https://svelte.dev/docs/svelte/svelte-compiler) turns components into JavaScript. `bun run build` bundles that code, the Svelte runtime code it needs, and the app's styles into the `.slop`. The interface keeps the Svelte version it was built with. The Mac app supplies the hitSlop document engine, the page shell and native services; it doesn't supply Svelte.

Svelte 5's [reactive updates](https://svelte.dev/docs/svelte/lifecycle-hooks) let a state change update the parts of an interface that depend on it. hitSlop's Svelte bindings connect those views to document snapshots. How much work an edit triggers depends on how the app is written.

Separate `App.svelte`, `Export.svelte`, and `Icon.svelte` components read the same document. hitSlop mounts capture views only when needed and handles saving and capture. The Tiny Wins example below shows how this works.

Svelte is our supported authoring integration. The document engine is framework independent, so another framework could use it through a new adapter. A React adapter, for example, would need to be implemented before it could offer the same authoring experience.

## A whole slop, from scratch

**Tiny Wins** is a complete example: a name, a counter, and a button for giving yourself a little credit. Its window, Finder icon, and export all read the same document.

With the same Bun and Mac app setup above, create a fresh starter:

```sh
bunx @hitslop/cli@4.0.0 init tiny-wins
cd tiny-wins
bun install
```

Replace the following starter files. Keep the generated `package.json` and `tsconfig.json`.

### 1. Give it a name and a window

`manifest.json` describes the app, including its starting window size.

```json
{
  "$schema": "https://api.hitslop.com/schemas/manifest.schema.json",
  "slug": "tiny-wins",
  "title": "Tiny Wins",
  "description": "A little credit for the things you get done.",
  "author": { "name": "You" },
  "categories": ["personal"],
  "presentation": { "width": 360, "height": 360 }
}
```

### 2. Say what it remembers

`schema.ts` defines the saved fields. Text is editable; a counter supports increments.

```ts
import { defineDocument, s } from "@hitslop/document";

export default defineDocument({
  title: s.text(),
  wins: s.counter(),
});
```

`initial.ts` supplies the starting values for **new** documents. Changing it later doesn't overwrite someone's saved wins.

```ts
export default { title: "Tiny wins today", wins: 0 };
```

### 3. Pick its colors

`theme.ts` declares the colors people can change, available as CSS variables; fonts and other styling stay in your CSS. The window, icon, and export use these colors. Anyone can change them from the window's theme panel, and share them as a theme file, and your agent can change them through hitSlop's theme commands, all without rebuilding. Changes stay with that document; the template keeps its defaults.

```ts
import { defineTheme } from "@hitslop/document";

export default defineTheme({
  surface: "#fff7e6",
  ink: "#382d24",
  accent: "#28634b",
});
```

### 4. Build the app, icon, and export together

`App.svelte` is the whole interface. Import the document from `schema.ts`, read from `doc.current`, write through `doc.fields`, and let hitSlop handle saving.

```svelte
<script lang="ts">
  import { bindText } from "@hitslop/document/svelte";
  import doc from "./schema";
</script>
<main class="wins-card">
  <input aria-label="Counter title" use:bindText={doc.fields.title} />
  <p class="wins-number" aria-live="polite">{doc.current.wins}</p>
  <button onclick={() => doc.fields.wins.increment()}>A little win +1</button>
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

`App.svelte` defines the window. The CLI discovers the two optional capture components:

- `Export.svelte` supplies the layout for previews and PNG/PDF exports. It reads the same document as the editor, but you can give it different markup and CSS. Here it shows the title and count without the input or button. Use normal document flow so long content can expand. Without this component, hitSlop captures the editor; mark controls with `data-slop-export="hide"` to leave them out.
- `Icon.svelte` supplies the document's dynamic Finder icon. hitSlop centers the artwork on a transparent 512 × 512 canvas. This example shows the saved count; another app could show a checklist's progress. Without an icon component, hitSlop uses its generic icon.

Click three times and the window shows **3**. Export a PNG or PDF and it shows **3** with your current title. When you close the document, hitSlop refreshes its Finder preview and icon from the saved data, so the icon shows **3** too. Register renders the template's initial artwork from starting values.

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

The builder connects `App.svelte` and `styles.css` to the host runtime automatically
through `defineSlop`; no `main.ts` is needed. For a non-Svelte app, an optional
`main.ts` can export `default { mount(ctx, target) }` and import its styles. See the
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

Then point the commands from [You and your agent, one document](#you-and-your-agent-one-document) at your document's path. With it open, the count ticks up and the accent turns purple, and the next export and icon capture use the new values. `slop theme reset` returns to the template's colors.

## Work on hitSlop

Use the Bun version pinned in `package.json`, Xcode, and XcodeGen on macOS:

```sh
bun install --frozen-lockfile
bun install --cwd apps/landing --frozen-lockfile
bun run build
bun run check
bun run test
bun run swift:test
bun slop dev examples/slops/quick-checklist
```

[Development](docs/guides/development.md) covers setup and adding templates, and the [documentation index](docs/README.md) covers how the platform works.

## Related projects

Other projects exploring personal software and interactive documents:

- [Hyperclay](https://hyperclay.com/): HTML files you can reshape in place, where the live document is the source of truth.
- [Capsule](https://withcapsule.app/): documents that run like apps, shared as single files.
- [bento](https://bento.page/): an office suite that fits in one self-contained HTML file.
- [Decker](https://beyondloom.com/decker/): HyperCard-style decks of interactive cards.
- [TiddlyWiki](https://tiddlywiki.com/): a personal wiki that lives in one HTML file.
- [uapp](https://thederf.com/uapp/demo)

Older inspirations: [HyperCard](https://www.computerhistory.org/revolution/the-web/20/373/2081) and [Smalltalk](https://squeak.org/) put making your own tools within reach. [Winamp's skin system](https://support.winamp.com/winamp-desktop-player-for-windows) was a major inspiration for hitSlop's look and feel.

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
