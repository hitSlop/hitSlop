# Windows and export

## Standard and responsive

`slop.ts`'s `window` width/height are the initial viewport. Standard windows may be
resizable, with `shape` as a CSS radius string (default `"22px"`) or an SVG path object.
Use `lockAspect: true` to preserve the initial width/height ratio. Build the outer
layout with grid/flex, relative units, min/max constraints, and container
queries. Test initial, narrower, and content-heavy states.

An unskinned slop may call `resizeWindow({ width, height })` from
`hitslop/svelte`. Never assume the screen can fit the request.

## The window is the stage

In every presentation mode the host resets `html`/`body` margins and makes them,
the automatic app root, and its mount ancestors fill the window. Framework-neutral apps
mark their root `data-hitslop-root`. Size the shell with `height: 100%`, grid, or
flex and scroll inside panes. Prefer these defaults over repeated `html`/`body`
sizing or `100vh`; override deliberately when the layout requires it. Native
masks radius/path shapes and skins, so keep controls inside the visible silhouette.
`slop dev` previews the `window` size and mask.

## Transparent backgrounds

Start with a radius/path shape and `background: "transparent"`. Transparent and
skin windows override ordinary `html`/`body` backgrounds with transparency;
draw the visible surface in your app, and avoid more-specific or `!important`
page backgrounds that would defeat that transparency.

The native host sets `data-slop-presentation` (`standard`, `transparent`, `glass`, `skin`),
`--slop-window-radius` for radius strings (`0` for paths), `data-slop-resizable` when enabled, and
`--slop-window-width`/`--slop-window-height` for initial dimensions. Use fluid CSS for live size.
The zero-specificity sizing rules are disabled during capture, so exports use normal flow.

Transparency alone does not create input holes. Keep focus rings and controls
inside the native silhouette. Move windows using the native toolbar handle;
there is no guest drag API or drag-attribute contract.

## Glass backgrounds

`background: "glass"` puts the system's frosted material behind the page and keeps the
page's own background. Paint `body` with a translucent theme color (`paper:
"#f6f3ee99"`) so the blurred desktop shows through, and people can tune it from the
theme panel. An opaque page background hides the glass. The frost stays light in dark
mode; a dark tint makes smoked glass. With Reduce Transparency on, macOS draws the frost
as a solid light surface. Keep text on a surface with
enough contrast over any desktop. Exports and icons can't capture the blur, so give
the `export` view a solid surface.

## PNG skins

Use SVG paths for geometric outlines and holes. Use a PNG for bitmap artwork. Import
the RGBA PNG and declare `window: { kind: "skin", width, height, image }`. The image is
exactly 1× or 2× the window size; prefer 2× for sharp edges.
The image is both native backing artwork and alpha mask. Alpha 0–25 is
click-through; 26–255 receives input. Skinned windows cannot resize. Avoid
critical controls on antialiased/translucent edges. Verify clicks actually reach
the application behind a hole, not merely that a DOM element ignores them.

## Static output

Register an optional `export` view in `defineSlop`. It receives
`mode: "preview" | "export"` and reads the same document by importing its module.
Render saved data in a fresh page; transient editor selection is not carried over. Share presentation and theme components.
Use normal flow rather than viewport heights or scrolling panels. This view also
supplies the catalog and Quick Look preview (captured at the export object’s
size, not the empty editor window). Without it, use `data-slop-capture="static"`
styles and `data-slop-export="hide"` on editing controls.

PNG exports use the window width and full content height at 2×, within 16384 pixels
per side and 24 megapixels. PDF retains selectable text on one content-sized page.
Dedicated exports do not inherit native window masks. Fonts, visible images, and
stable geometry are awaited; asynchronous charts can use `capture.onPrepare`.

Rendering failures in export or icon components reject that capture without
replacing the editor or reporting an application-render error. Restoration
clears the component failure so the next attempt renders it again. Editor rendering
failures still use native application-error recovery and prevent capture.

## Icon

Register an optional `icon` view in `defineSlop`. It mounts
only for capture, centered on a transparent 512×512 surface. Pass progress or other saved data if useful; keep
a strong silhouette, safe margins, and no essential small text. It refreshes
a document's artwork and Finder icon on close; the template's icon remains as built.
Build with `--artwork native` (or register) and check the generated artwork (`slop inspect` lists it; Quick Look shows it), then verify native exports.
