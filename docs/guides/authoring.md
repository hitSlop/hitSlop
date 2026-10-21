# Authoring templates

The public guides are the how-to for every author, including us. This page adds the
rules and standards for templates in this repository.

| Topic | Guide |
| --- | --- |
| First slop, end to end | [Create your first slop](../../apps/landing/src/content/docs/docs/getting-started.mdx) |
| Fields, handles, changes and previews | [Data and schemas](../../apps/landing/src/content/docs/docs/guides/data-and-schemas.mdx) |
| Saved state, lifecycle, attachments, HTTPS and YouTube | [Files and the web](../../apps/landing/src/content/docs/docs/guides/files-and-web.mdx) |
| App definition, window shapes and hover controls | [App definition and windows](../../apps/landing/src/content/docs/docs/guides/manifest-and-windows.mdx), [PNG window skins](../../apps/landing/src/content/docs/docs/guides/png-window-skins.mdx) |
| Plain CSS and the theme palette | [Style a slop](../../apps/landing/src/content/docs/docs/guides/styling.mdx) |
| Export views, icons and capture | [Icons, previews, and exports](../../apps/landing/src/content/docs/docs/guides/icons-and-exports.mdx) |
| Build, register and share | [Build and share](../../apps/landing/src/content/docs/docs/guides/build-and-share.mdx) |
| Every document kind, error code and CLI path | [Document types](../reference/document-types.md) |
| The app interface (`ctx`) for other frameworks | [Runtime reference](../reference/runtime.md#page-shell-and-ctx) |

## Work in this checkout

Use `bun slop dev slops/SLUG`, `bun slop build SOURCE` and
`bun slop register SOURCE`. [Development](development.md) covers preparing the tools,
adding a template and choosing which templates ship.

## Rules for authored code

- Read `slop.ts` first. One slop does one understandable job.
- Apps never import the runtime, call the host bridge, write SQLite or keep a second JSON
  copy of the document.
- Save only what is worth keeping. Selection, hover, drag positions and timers stay in
  Svelte `$state`; files are attachments, never base64 in fields.
- Key rows by `$id`. Don't write in `$effect` or on mount; defaults belong in `schema.ts` and its
  `initial` option, which seeds new documents only. Changing the schema makes a new document type.
- Don't silence writes with `.catch(() => {})`: the host reports a refused write, and the
  window shows a page handler's `refuse(message)` in its notice region. Catch only to react in
  the template's own way. Order rows with `move`, not a position field.
- Declare a scalar's `default` in the schema instead of repeating it at every insert.
- Use `EditableText` for text, `draft` for composers, `motion` for animated numbers and
  `notify` for messages (all from `hitslop/svelte`) rather than reimplementing them.
- Use required `Preview.svelte` for a compact overview and optional `Export.svelte` with natural height for complete growing content; derive their
  data from the `snapshot` prop and keep editor controls and effects out of it. All capture
  views must render synchronously without browser APIs.
- Import optional `hitslop/base.css` before app styles for low-specificity form, focus
  and reduced-motion defaults. App CSS still owns typography, layout and palette. Of two
  equally specific rules, the later import wins. A capture's CSS is exactly what its own
  Preview/Export/Icon/Widget role imports, in import order, including component CSS.
  Captures no longer inherit the editor's stylesheets; import shared CSS in each role
  or a component it imports. `slop dev` still mounts static views inside the editor page.
- Optional-object handles use `$set(value)` and `$clear()` because they also carry
  authored fields; scalar and text handles keep `set` and `clear`.
- Build and register render every declared view during pack, including Export PNG/PDF
  and all Widget families, even with supplied artwork. A failed render publishes nothing.
- Capture views never change saved state to prepare a view.

For shared editing controls in static views, follow the platform's
[hide rule](../../apps/landing/src/content/docs/docs/guides/icons-and-exports.mdx#make-exports-readable).

## Design standards

The examples are references, not a limit or a default visual
style. Start each template from its own purpose. Paper, Instrument and Skin are optional
directions. `_vibe/` is inspiration only; never ship its images. Record product context in
[PRODUCT.md](../../slops/PRODUCT.md) and each object's actual palette, typography,
layout, state language and motion in its `DESIGN.md`.

Make the purpose visible in the first viewport. Keep controls familiar, state readable
through words and structure, and essential text comfortable. Start with realistic content
at the declared window size; remove competing elements before shrinking labels. Aim for at least
12px supporting text, 14px control labels and 44px action targets. Provide keyboard access,
visible focus, sufficient contrast and reduced-motion behavior.

The host disables ordinary text selection in the editor by default; inputs, textareas
(including readonly fields), and editable content retain normal selection. Enable
`-webkit-user-select: text; user-select: text` on useful copyable content such as notes,
addresses, or code, or on `body` for the whole slop. The default also applies in
`slop dev` and leaves captures alone. See the
[styling guide](../../apps/landing/src/content/docs/docs/guides/styling.mdx#text-selection)
for CSS overrides.

Motion should explain change. Static views render saved targets directly. Persist target values
immediately and honor reduced motion. Stop transient work on unmount and pause it while
the window is hidden, as
[Files and the web](../../apps/landing/src/content/docs/docs/guides/files-and-web.mdx#opening-hiding-and-closing)
describes.

## Review a complete object

Review empty, typical, long-content, failed and busy states; keyboard and IME interaction;
reduced motion; initial and narrow widths; theme overrides; and open overlays. Use normal
controls for temporary review data rather than changing creation defaults.

Run check and build, register, create a writable copy, type then immediately close,
reopen, duplicate, and export PNG and PDF. Inspect the editor, export and icon from the
same revision. Review persistence, clipping, skins and desktop click-through in the
native app. **Do not create an automated test or test suite for each slop.** Creating,
styling, animating, or updating a slop requires existing checks/builds and hands-on
review, not new example-specific tests. Shared SDK, storage, or host regressions belong
in the existing tests at their owning boundary. See [testing](../testing.md),
[presentation fixtures](development.md#focused-checks) and the
packaged [design skill](../../packages/hitslop/skills/hitslop-design/SKILL.md).

## Commands

Keep the stored shape and initial values in `schema.ts`. Optional `commands.ts`
default-exports one `defineCommands` set for CLI and agent callers:

```ts
import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import schema from "./schema";

export default defineCommands(schema, {
  rename: {
    description: "Change the title shown in the window.",
    args: { title: s.string({ minLength: 1 }) },
    run({ tx }, { title }) { tx.fields.title.set(title); },
  },
});
```

Only root `commands.ts` may import `hitslop/commands`, and the page cannot import
`commands.ts`. The page edits with handles and `doc.change()`. To give the page and an
agent the same edit, write it once over `Tx<typeof schema>` in a plain module:

```ts
// tasks.ts
import type { Tx } from "hitslop";
import type schema from "./schema";
export const rename = (tx: Tx<typeof schema>, title: string) => tx.fields.title.set(title);
```

The page calls `doc.change(tx => rename(tx, title))`; the command's `run` calls
`rename(tx, title)`.
The [public command walkthrough](../../apps/landing/src/content/docs/docs/guides/edit-installed-data.mdx)
shows discovery with `slop describe My.slop` and invocation with
`slop call My.slop rename --args '{"title":"Weekend"}'`. A command without a return
value or inserts prints `{"result":null,"ids":[]}`.

Commands synchronously read immutable `ctx.current`, collect writes with `ctx.tx`,
and return JSON or throw. Use `ctx.now` and `ctx.random()`; no ambient clock, randomness,
browser APIs, I/O, promises or generators. Module globals start fresh on every call.
Reading the mounted document inside a command is refused: use the supplied context.

Arguments are a flat object of `s.string`, `s.number`, `s.integer`, `s.boolean` or
`s.enum`, optionally wrapped in `s.optional`. Defaults are filled by Rust; unknown
arguments are refused. Pass a row's `$id` as `s.string()` and address it through
`tx.fields.tasks.item(id)` or `remove(id)`. A missing row rejects the entire edit.
String bounds count Unicode code points; DOM offsets count UTF-16 units.

A successful command is one undo step and completes after saving. A definite stale
conflict retries once with the same clock and seed; an unknown outcome never retries.
`refuse(message)` returns the authored message with reason `refused` and changes nothing.

Command descriptors live in `definition_json`; `commands.js` is private and immutable.
JSON Schema is computed only for `describe`, never stored or accepted as input.
Installing hitSlop does not replace a document's embedded app.

## Platform, device and responsive layout

The public guide [Make it work on iPhone](../../apps/landing/src/content/docs/docs/guides/mobile.mdx)
is the author-facing walkthrough of this section: compact layouts, safe areas, touch
targets, haptics and testing. Keep it and this section in step.

### Haptic feedback

`haptics` from `hitslop/svelte` plays short native feedback for deliberate editor
actions. `haptics.selection()` accompanies a changed selection;
`haptics.impact()` defaults to `"light"` and also accepts `"medium"`, `"heavy"`,
`"soft"` and `"rigid"`. `haptics.notification("success" | "warning" | "error")`
accompanies an explicit outcome, alongside visible feedback:

```ts
await doc.change(tx => addEntry(tx, input));
haptics.notification("success");
```

These methods return `void`; never await them or make an edit depend on feedback.
The framework-neutral host service is optional `ctx.haptics`. Older hosts, desktop,
browser previews and unsupported hardware stay silent. No capability declaration
or permission prompt is needed. The iPhone Settings toggle controls all documents;
inactive, hidden and closing editors do not play feedback. Keep haptics out of
document observers, typing, rendering, static views and background work.

### Surface motion

`surfaceTransition`, `overlayTransition` and `popoverTransition` from
`hitslop/svelte` are Svelte transition functions with reduced-motion support. Use
`transition:surfaceTransition={{ kind: "sheet" }}` for bottom sheets (the default
kind is `"dialog"`), `transition:overlayTransition` for the dimming layer and
`transition:popoverTransition` for menus/selects. They preserve existing transforms.

Use Bits UI's `forceMount` and `child` snippet, rendering the supplied `props` on
the transitioning DOM element inside `{#if open}`. Floating components also supply
`wrapperProps`: put those on an outer wrapper and animate the content inside it.
Keep menu item data through the outro, and use `onOpenChangeComplete` to open a
dialog after its menu closes. For rapid reopening during an exit, keep a stable
opener reference and restore it in `onCloseAutoFocus` with `preventScroll: true`;
the previously focused element can otherwise belong to the retiring dialog.
Keep focus management with Bits UI; opening a phone
sheet should focus its container without bringing up the keyboard. Prefer one
modal with internal screens for related forms, retaining draft values outside the
screen block. Animate keyed list reordering with Svelte's `animate:flip` and honor
`prefersReducedMotion.current` for any additional JavaScript motion.

### Environment

Call `getEnvironment()` from `hitslop/svelte` during editor component initialization,
just like `getDocument(schema)`. Its read-only `current` snapshot is reactive:

```svelte
<script lang="ts">
  import { getEnvironment } from "hitslop/svelte";
  const environment = getEnvironment();
</script>

{#if environment.current.platform === "macos"}
  <p>Mac-specific controls go here.</p>
{/if}
{#if environment.current.layout === "compact"}
  <p>A composition for a narrow window.</p>
{/if}
```

`platform` is `macos`, `ios`, `android`, `windows`, `linux` or `unknown`.
`device` is `phone`, `tablet`, `desktop` or `unknown`. Native hosts supply identity;
browser hosts and older hosts report `unknown`. Android and Windows hosts are future
work. A device category is a presentation hint, not a permission or capability check.

`layout` is `compact` below 640 CSS pixels of the actual document viewport and
`regular` otherwise. A narrow Mac window remains a desktop device. Layout updates on
resize and rotation without editing saved state. Read `environment.current` inside
reactive expressions; destructuring it once keeps that old snapshot.

CSS receives the same editor information on `<html>`:

```css
:global(html[data-slop-layout="compact"]) .controls { flex-direction: column; }
:global(html[data-slop-platform="macos"]) .mac-controls { display: flex; }
```

Use plain `html[...]` selectors in standalone CSS. Keep base styles usable without
these attributes for older hosts; ordinary media and container queries also work.
Choose responsive layout for available space and pointer/hover media queries for input
differences. Full-screen content can use `env(safe-area-inset-top, 0px)` and the other
safe-area insets for padding around system areas; the page background can still fill
the display. These insets do not describe floating host controls.

`slop dev` offers Browser, iPhone, iPad and Mac device presets. Phone/tablet presets
change the viewport; Browser and Mac restore the authored size. Changes preserve
accepted edits and simulate presentation only. Preview, Export, Icon and Widget remain
static views of their capture props and never read `getEnvironment()` or receive these
editor attributes.

### iPhone editing

The current iPhone app is portrait-only. Its editor fits below native navigation,
above the home indicator, and above the onscreen keyboard; the background fills the
screen. Use `height: 100%` with `min-height: 0` and scrollable content panes rather
than subtracting keyboard height or adding a second set of native safe-area insets.
The native Done action dismisses the keyboard without submitting a form. The host suppresses tap highlights and defaults buttons, links and button-role controls to `touch-action: manipulation` on iOS, using overridable styles. Page zoom
and input-focus zoom are disabled in the native iPhone editor; authored canvas or
image gestures can still implement their own zoom.

For compact editors, use at least 16px text in inputs and 44px touch targets (the
visible icon may be smaller). Set `inputmode`, `enterkeyhint`, `autocapitalize`, and
`autocomplete` to match the field. Keep multiline Return and IME composition intact.
Repeated-entry tools should retain focus after Add; destructive or unrelated actions
should not summon the keyboard. Do not globally cancel touch events or text selection.

Use Bits UI dialogs for mobile sheets. Bound them to the available viewport, keep
close and submit controls reachable, and scroll their content. On opening, focus the
sheet rather than a text field when keyboard entry is optional; retain its focus trap
and restoration. Test actual keyboard transitions on iPhone: device presets simulate
layout and identity, not the native keyboard, navigation, or permissions.

## Entry and media

`slop.ts` exports metadata through `defineSlop({ slug, title, description, author,
categories, window, theme, artwork?, capabilities? })`. Required `schema.ts` default-exports
`defineDocument(fields, { initial })`; `initial` is optional when all fields have defaults
or may be omitted. Required `App.svelte` is the editor; required `Preview.svelte` supplies compact preview artwork, and required `Icon.svelte` supplies the icon. `Export.svelte`, `Widget.svelte` and `commands.ts` are optional. These root names are case-sensitive. Import CSS from
components. The page imports `schema.ts`, so
its definition ships in the editor bundle; metadata and command bodies do not.
The window is either `{ kind: "standard", width, height, ... }` or
`{ kind: "skin", width, height, image: importedPNG }`.

Both accept `fullscreenable: true` (default false), independently of `resizable`.
This offers the native fullscreen button and View menu action; it never enters
fullscreen automatically. A resizable standard window without an explicit shape or
aspect lock fills the screen. Fixed, shaped, aspect-locked and PNG-skinned windows
retain their authored composition, scaled to fit on a black backdrop. The live view
stays mounted; fullscreen is not an export view or a saved document setting.
Fitted pages render with host zoom so text stays sharp. PNG skins keep their source
resolution when enlarged; use a larger skin image for large displays, or a vector
window shape for a resolution-independent outline.

Import template images and fonts in components, CSS `url()`s or the declaration. Vite
collects the real bundle and rewrites them to `/assets/media/<sha256>.<ext>` URLs; duplicates
share one resource. A skin or export-only image is included even if the editor never
renders it. `public/` is not copied; import those files explicitly. Supplied artwork uses
`artwork: { preview: importedPNG, icon: importedPNG }`. A skin must be RGBA at 1× or 2×
the logical window size; Rust validates its pixels before packing.

User-imported images are attachments, separate from template assets. Import `attachments`
from `hitslop/svelte`, use `attachments.import(file, (tx, ref) => { ... })`, store the
reference, and render `attachments.url(ref.id)` in an image or media element. `read(id)` fetches a Blob when
code needs bytes. The host supplies the media type and serves ranges without a base64
read bridge. Attachments stay with the document when it is copied or shared.

The [capability guide](../reference/runtime.md#security-boundaries) records the permission boundary and links to
complete public examples. A slop that uses the camera or microphone declares it: `capabilities: ["camera",
"microphone"]`. Then use the web APIs directly (`navigator.mediaDevices.getUserMedia`,
`MediaRecorder`, canvas). The host refuses a device the slop did not declare, and macOS asks
the person once for hitSlop. Start devices from a button, show that capture is on, handle
`NotAllowedError`, and stop every track when finished. Keep live streams and unfinished
recordings in view state; keep a finished photo or clip by importing it as an attachment,
wrapping the Blob in a File for its name: `new File([blob], "photo.jpg", { type: blob.type })`.
One attachment holds at most 10 MiB (about ten minutes of 128 kbit/s audio). Export and
icon views render saved content and never request devices.

To turn a kept recording into text, declare `"speech"` and pass its attachment reference
(or ID) to `speech.transcribe` from `hitslop/svelte`; the recording is never sent again
through the page. Recognition runs on the Mac only: a locale the Mac can recognize only
through Apple's servers is unavailable, never uploaded. Saving the text is an ordinary
edit.

```ts
import { attachments, isSpeechError, speech } from "hitslop/svelte";

const ref = await attachments.import(new File([blob], "note.m4a", { type: blob.type }), (tx, ref) => {
  tx.fields.notes.insert({ clip: ref.id, text: "" });
});
try {
  const { text } = await speech.transcribe(ref, { locale: "en-US" });
  // write `text` into the note with doc.change
} catch (error) {
  if (!isSpeechError(error)) throw error;
  // error.code: "unavailable" (see error.reason), "cancelled" or "failed"; show error.message
}
```

`locale` is optional (the person's own when absent) and returns as the locale used.
Rust normalizes `_` separators to `-` before dispatch. The Mac provider transcribes
m4a, mp3, wav, flac, mp4 and mov. It returns `unavailable` with reason `format` for
other formats, including Ogg and WebM, before asking for permission. Let the provider
decide format support rather than keeping a format list in authored code. A transcript holds at most
1 MiB of text. `unavailable` carries a `reason`: `notDeclared` (no `"speech"` in
`capabilities`), `host` (`slop dev`, a browser copy, an export, or an older hitSlop),
`system` (the Mac cannot recognize speech now), `locale` (no on-device recognizer for the
locale), `permission` (the person declined speech recognition for hitSlop), or
`format` (this provider cannot transcribe the recording format). `cancelled`
means the window closed, the interface reloaded or an export began first; `failed` means
the recording could not be read or recognized. Keep a usable path without a transcript.


### Widget authoring

Optional `Widget.svelte` receives `WidgetProps` from `hitslop/svelte`: `family`, `width`, `height`.
It draws saved document state and theme into a fixed canvas; content outside the canvas is clipped.
Use the `slop dev` Preview selector for Editor, Preview, Icon, Small widget, Medium widget and Large widget.
Switching preserves accepted edits; component and CSS changes update the selected view live.
The browser preview helps compose each view, but native output uses Takumi and can differ.
Review an actual native capture before shipping. The Mac app captures all three sizes after
successful saves (coalesced for one second), on opening when cached state is stale or missing, and on close
after edits. While hitSlop runs it also watches cached source files for closed-document
edits, and checks for missed edits on startup. Renderer and bundled-font updates
also invalidate cached images without a document edit. Quitting retains the last image.
A failed refresh keeps the previous generation. Publishing replaces the complete image generation, then requests a WidgetKit
reload; the OS decides when that reload is displayed. Optional `widget.refreshMinutes` in `slop.ts`
requests approximate refresh while the app runs; authoring accepts 15–1440 minutes.

The iPhone TestFlight preview renders placed library slops locally with the same
`Widget.svelte` views and canvases; taps open the slop. Foreground saves and library
changes refresh images. CloudKit notifications and scheduled background work also
refresh placed widgets when iOS allows; neither gives an exact refresh time. See the
[widget architecture](../architecture.md#macos-image-widgets).

Widget rendering uses synchronous Svelte SSR in an isolated Rust child on Mac and
the in-process Rust capture pipeline on iPhone. Read the `snapshot` prop;
document handles and writes are unavailable. Do not rely on `onMount`, DOM measurement,
canvas, browser globals or async/await for widget content.
Use font longhands, flexbox or numeric grid placement, and SVG attributes/currentColor.
The build collects CSS; omit `css="injected"` from Svelte options. Native capture supports bundled PNG/JPEG/GIF/WebP and SVG images, plus referenced
image attachments through the `attachmentURL(id)` prop. Blob reads/imports and
remote loading are unavailable in capture views.
Named grid areas are supported by the pinned renderer. Container queries and page CSS
inside inline SVG are not portable.
Like Takumi's HTML API, native capture ignores unsupported CSS declarations and rules;
review the native images, since a successful capture does not guarantee browser fidelity.
All icons, previews, exports and widgets use this constrained renderer. `Preview.svelte`
and `Icon.svelte` are required at authoring; `Export.svelte` is optional; browser-dependent capture closures fail the build with a
role/source diagnostic. PNG and selectable vector PDF share native layout. There is no
public renderer override or automatic fallback.

The renderer bundles Geist (sans), Newsreader (serif), Geist Mono, converted Apple color emoji and Noto Sans Symbols 2 for text symbols.
It does not discover platform fonts. Import additional fonts with `@font-face` and CSS
`url()`; the document packages their bytes and the renderer registers their family aliases,
weights and normal/italic styles for each batch. Variable font ranges preserve the font's
intrinsic weight range. You may switch between packaged families based on saved state.
`local()`-only or remote font sources cannot render. Other scripts need a bundled font
with the required glyphs; the default set does not promise universal Unicode coverage.
Vector PDF refuses uncovered characters with a diagnostic naming the missing glyphs.
