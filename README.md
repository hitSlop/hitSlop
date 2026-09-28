# hitSlop

**Tiny apps. Big personality.**

Build mini apps for your Mac with Svelte and your favorite AI coding agent. Make a planner that fits your week, an interactive document, or a weird desktop experiment you've been thinking about.

Your agent can work with the data, too. It can read changes you make in the app, and its edits through the document CLI appear in the open window. Each `.slop` keeps the interface and saved work together in one portable package. Local apps work offline, and you can send a copy to a friend with hitSlop.

[Download for Mac](https://github.com/hitslop/hitslop/releases/latest/download/hitSlop.dmg) · [Explore the website](https://hitslop.com) · [Make your first slop](#make-your-own-with-an-agent) · [Docs](docs/README.md)

macOS 15.2+ · Apple silicon · No account required

<p align="center">
  <a href="apps/landing/public/assets/desktop-hero.mp4"><img src="apps/landing/public/assets/desktop-hero-poster.jpg" width="900" alt="A Mac desktop full of open slops: a Winamp-style music player, a desktop pet, a focus timer, flashcards, a doodle board, a koi pond, Wordle, and school planners."></a>
  <br><sub>▶ <a href="apps/landing/public/assets/desktop-hero.mp4">Watch the 30-second tour</a></sub>
</p>

## Small enough to be yours

Sometimes you just want a packing list for one trip. A timer that looks like a tomato. A recipe card covered in your own notes. A tiny pond to stare at between meetings.

Those are good reasons to make software.

hitSlop is built for tools with one clear job and a little character. Start with a template, or ask your coding agent to make the thing you keep wishing existed. Give it the fields you need, the colors you like, and a window that fits the job. It can feel like a sheet of paper, a pocket calculator, or something you found in an old arcade.

If all you need is a packing list for next weekend, that's enough.

## What belongs on your desktop?

This development branch currently ships [Quick Checklist](examples/slops/quick-checklist). Other examples are preserved in [the slop archive](archive/slops) while their data types are ported.

## Keep the app. Keep the work.

- Your documents live on your Mac. You don't need an account or a server to use a local slop.
- The interface, saved data, and imported attachments travel together. Close a document before moving or copying it in Finder.
- Send a friend a template or a copy of a finished document. Zip the closed `.slop` package for transfer; they'll need a compatible hitSlop Mac app. [How to share a slop](apps/landing/src/content/docs/docs/guides/build-and-share.mdx#share-a-template-or-a-document).
- Use the app yourself or have an agent edit its data through the document CLI. Both work with the same document. The installed native CLI works without Node or Bun.
- Export a PNG or PDF to send an invoice, print a recipe, or drop a plan into a message. Authors can define a separate export layout with its own styling and no editing controls.
- Dynamic Finder icons can show what's in a document, such as a counter's saved total. hitSlop refreshes the icon from saved data when you close the document.
- Give each app its own look. Change a document's supported theme colors, or edit the source to make a different tool.

## Why Svelte?

Svelte suits the kinds of apps we want to make: small tools with custom interfaces that are easy to work on.

You can keep a component's markup, behavior, and [scoped CSS](https://svelte.dev/docs/svelte/scoped-styles) together in a `.svelte` file. That makes a small interface easy to follow and gives you room to design a paper planner, a calculator, or a pond without adopting a prescribed set of UI components. TypeScript and `bun run check` help catch type and template mistakes as you work.

[Svelte's compiler](https://svelte.dev/docs/svelte/svelte-compiler) turns components into JavaScript. `bun run build` bundles that code, the Svelte runtime code it needs, and the app's styles into the `.slop`. The interface keeps the Svelte version it was built with. The Mac app supplies the hitSlop document engine, the page shell and native services; it doesn't supply Svelte.

Svelte 5's [reactive updates](https://svelte.dev/docs/svelte/lifecycle-hooks) let a state change update the parts of an interface that depend on it. hitSlop's Svelte bindings connect those views to document snapshots. How much work an edit triggers depends on how the app is written.

[Snippets](https://svelte.dev/docs/svelte/snippet) also fit our window, icon, and export model. With hitSlop's `<Slop>` component, you can define each view alongside the others, all reading the same document. hitSlop handles saving and capture. The Tiny Wins example below shows how this works.

Svelte is our supported authoring integration. The document engine is framework independent, so another framework could use it through a new adapter. A React adapter, for example, would need to be implemented before it could offer the same authoring experience.

## Make your own with an agent

Start with an idea and a coding agent. Setup asks what your slop should do and who made it, then offers to launch your agent's CLI. The starter includes a working Svelte checklist, your `BRIEF.md`, `AGENTS.md`, and authoring/design skills.

Use Bun 1.4.2 or newer and the matching hitSlop Mac app, installed in `/Applications` or `~/Applications` for native builds and registration.

**1. Give your idea a folder.**

```sh
bunx @hitslop/cli@1.2.0 init weekend-kit
cd weekend-kit
bun install
```

**2. Let your chosen agent build it.**

At the end of setup, choose a detected agent, **Other CLI…** to enter another installed tool, or **Finish without launching**. The agent opens in the project with your brief and hitSlop guidance. Use `--yes` and `--brief 'What to build'` when an agent or script is already creating the project; this never launches another agent.

You can also open the folder in your agent yourself. Try this:

> Read AGENTS.md, manifest.json, and the authoring/design skills in .agents/skills first. Turn this starter into "Weekend Kit," a packing list for short trips. Let me add items, group them by bag, check them off, and see how many are left. Make it feel like a pocket field notebook: warm paper, forest-green ink, and comfortable checkboxes. Include a clean printable packing list for PNG/PDF export. Keep it small, use hitSlop's document APIs for saved data, and run the project checks when you're done.

Try it and ask for changes as you go: "Make the checkboxes bigger" or "Let me rename the trip." It's easier to judge what works once you can use it.

**3. Try it, then put it on your desktop.**

```sh
bun run dev
```

The browser preview uses disposable data; refreshing resets it. Restart the dev command after source changes. When it feels right, stop the preview and run:

```sh
bun run check     # Check your Svelte and TypeScript code
bun run build     # Package your app as dist/weekend-kit.slop
bun run register  # Add the template to hitSlop's local catalog
```

Open hitSlop, choose Weekend Kit under **Templates**, and select **Create** to make your own writable document. Add a few items, close and reopen it, then try a PNG or PDF export.

You can also zip up `dist/weekend-kit.slop` and send it to a friend. They just unzip it and open the `.slop` with hitSlop on their Mac. The app and its starting content are included; your personal saved data stays in your document.

You keep the source and the finished app. Make another version whenever you like; existing documents keep the app version they were created with.

[Follow the full tutorial](apps/landing/src/content/docs/docs/getting-started.mdx) · [Authoring guide](docs/guides/authoring.md) · [Edit a document with your agent](apps/landing/src/content/docs/docs/guides/edit-installed-data.mdx)

## A whole slop, from scratch

**Tiny Wins** is a complete example: a name, a counter, and a button for giving yourself a little credit. Its window, Finder icon, and export all read the same document.

With the same Bun and Mac app setup above, create a fresh starter:

```sh
bunx @hitslop/cli@1.2.0 init tiny-wins
cd tiny-wins
bun install
```

Replace the following starter files. Keep the generated `package.json` and `tsconfig.json`.

### 1. Give it a name and a window

`manifest.json` describes the app, including its starting window size.

```json
{
  "$schema": "https://api.hitslop.com/schemas/v1/manifest.schema.json",
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

`theme.ts` declares tokens available as CSS variables. The window, icon, and export use these colors. Your agent can change them through hitSlop's theme commands while the document is open, without rebuilding. Overrides stay with that document; the template keeps its defaults.

```ts
import { defineTheme } from "@hitslop/document/theme";

export default defineTheme({
  surface: "#fff7e6",
  ink: "#382d24",
  accent: "#28634b",
});
```

### 4. Build the app, icon, and export together

`App.svelte` is the whole interface. Read from `doc.current`, write through `doc.fields`, and let hitSlop handle saving.

```svelte
<script lang="ts">
  import { Slop, useDocument, bindText } from "@hitslop/document/svelte";
  import schema from "./schema";

  const doc = useDocument(schema);
</script>

<Slop>
  <main class="wins-card">
    <input aria-label="Counter title" use:bindText={doc.fields.title} />
    <p class="wins-number" aria-live="polite">{doc.current.wins}</p>
    <button onclick={() => doc.fields.wins.increment().catch(() => {})}>A little win +1</button>
  </main>

  {#snippet icon()}
    <div class="wins-icon">{doc.current.wins}</div>
  {/snippet}

  {#snippet exportView()}
    <article class="wins-card">
      <h1>{doc.current.title}</h1>
      <p class="wins-number">{doc.current.wins}</p>
      <p>Little things add up.</p>
    </article>
  {/snippet}
</Slop>
```

The markup directly inside `<Slop>` defines the window. The two named snippets define what you see outside it:

- `{#snippet exportView()}` supplies the layout for previews and PNG/PDF exports. It reads the same document as the editor, but you can give it different markup and CSS. Here it shows the title and count without the input or button. Use normal document flow so long content can expand. Without this snippet, hitSlop captures the editor; mark controls with `data-slop-export="hide"` to leave them out.
- `{#snippet icon()}` supplies the document's dynamic Finder icon. hitSlop centers the artwork on a transparent 512 × 512 canvas. This example shows the saved count; another app could show a checklist's progress. Without an icon snippet, hitSlop uses its generic icon.

Click three times and the window shows **3**. Export a PNG or PDF and it shows **3** with your current title. When you close the document, hitSlop refreshes its Finder preview and icon from the saved data, so the icon shows **3** too. Build/register generate the template's initial artwork from starting values.

See [Export and icons](docs/guides/authoring.md#export-and-icons) for capture details and size limits.

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
[authoring guide](docs/guides/authoring.md) for the app interface.

### 5. Take it for a spin

```sh
bun run check     # Check the types and Svelte component
bun run dev       # Try it in the browser; preview data resets on refresh
```

Stop the preview before continuing. Restart it after source changes.

```sh
bun run build     # Create dist/tiny-wins.slop, including preview and icon artwork
bun run register  # Add Tiny Wins to your local template catalog
```

In hitSlop, choose **Tiny Wins → Create**, then save your document as `My Wins.slop`. Change its title, add some wins, export a PNG/PDF, and close and reopen it to see the saved values. Check its refreshed icon in Finder, too.

Your agent can add a win to that same document through the CLI. Substitute the path where you saved it:

```sh
bunx @hitslop/cli@1.2.0 get "/path/to/My Wins.slop"
bunx @hitslop/cli@1.2.0 apply "/path/to/My Wins.slop" --op '{"type":"increment","path":["wins"],"by":1}'
```

With the document open, the number changes in the window. Its next export and icon capture use the updated value as well.

Ask your agent to "make the accent purple," and it can change the declared theme token at runtime:

```sh
bunx @hitslop/cli@1.2.0 theme set "/path/to/My Wins.slop" --values '{"accent":"#7050ad"}'
```

The open window updates immediately, and the next export and icon capture use the same purple. To return to the template's colors:

```sh
bunx @hitslop/cli@1.2.0 theme reset "/path/to/My Wins.slop"
```

## Use the CLI

Use `bunx @hitslop/cli@1.2.0` for individual commands, or install with `bun install -g @hitslop/cli@1.2.0` and run `slop` from Bun's PATH. In this checkout, use `bun slop`. Generated projects provide their own pinned `bun run` scripts.

| Task | Commands |
| --- | --- |
| Create and preview an app | `init SOURCE`, then the project's `bun run check` and `bun run dev` |
| Package and install a template | `bun run build`, `bun run register` |
| Inspect and edit a writable document | `schema`, `get`, `apply`, `batch` |
| Import complete JSON data | `import --from`, or `get --snapshot` followed by `import --replace --if-version` |
| Customize colors and manage files | `theme get/set/reset`, `attachments list/import/export` |
| Export a PNG or PDF | `export DOCUMENT --format FORMAT --output FILE` (`png` or `pdf`) |
| Install agent guidance | `skills install`, `skills repair`, `skills uninstall` |

The table lists command families; follow the [CLI workflows](apps/landing/src/content/docs/docs/guides/cli-workflows.mdx) for complete examples and required arguments. The [repository CLI reference](docs/guides/cli.md) covers operation shapes and contributor details. Add `--help` to a command to see its arguments.

Native document commands run on macOS through the installed app. You can invoke `"/Applications/hitSlop.app/Contents/Helpers/hitslop-native"` directly without Node or Bun; its `create` and `open` commands make and open writable documents. Built and registered templates remain immutable. After an uncertain edit result, inspect with `get` before another edit.

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

[Development](docs/guides/development.md) covers setup, the workspace, and adding templates. All active templates are discovered for checks and `bun run build:templates`; `examples/slops/bundled.json` selects those shipped with the app. The local catalog also discovers templates under `~/.hitslop/templates`.

The native Mac client supplies the document engine and local persistence. Authors build with the matching `@hitslop/document`, `@hitslop/schema`, and `@hitslop/cli` packages.

Hosted discovery, public template publication, accounts, collaboration, and iCloud/synced folders are deferred. The current workflow is local creation, editing, and export.

Start with the [documentation index](docs/README.md). Contributors run `bun run test:local` before release. See [releasing](docs/guides/releasing.md) for signed-app and npm publication steps.

## License

MIT © 2026 hitSlop contributors. See [LICENSE](LICENSE) and [third-party notices](THIRD_PARTY_NOTICES.md).

## Related projects

Other projects exploring personal software and interactive documents:

- [Hyperclay](https://hyperclay.com/)
- [Capsule](https://withcapsule.app/)
- [uapp](https://thederf.com/uapp/demo)
- [bento](https://bento.page/)
- [Decker](https://beyondloom.com/decker/)
- [TiddlyWiki](https://tiddlywiki.com/)

Older inspirations include [HyperCard](https://www.computerhistory.org/revolution/the-web/20/373/2081) and [Smalltalk](https://squeak.org/), which put making your own tools within reach. [Winamp's skin system](https://support.winamp.com/winamp-desktop-player-for-windows) was a major inspiration for hitSlop's look and feel.
