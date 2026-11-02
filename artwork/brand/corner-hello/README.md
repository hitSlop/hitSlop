# Corner Hello

The selected dotpop character: an ivory file, an expressive magenta fold, unequal eyes,
and a small smile. `build.mjs` is the one geometry source for the SVG masters and RML.
The app icon and each document's artwork are separate identities and remain unchanged.

```sh
node artwork/brand/corner-hello/build.mjs
rive artwork/brand/corner-hello --verify
rive inspect artwork/brand/corner-hello --summary
rive artwork/brand/corner-hello
```

The preview window watches the project. For a runtime build and shared native/site
artwork exports, run `node artwork/brand/corner-hello/export.mjs`. Set `RIVE_CLI` if
the installed CLI is outside PATH. Exports use the repository's Playwright dependency,
installed Chrome to rasterize SVG, and ImageMagick for PNG/WebP encoding. Rive itself
builds the `.riv`; no browser runtime previews the Rive asset. The runtime asset contains no
scripts, external fonts, audio, raster textures or network requests and builds locally
without publishing or signing in.

The `Launch` state machine on the `Corner Hello` artboard has three states:

- **Hello:** 48 frames at 60 fps. The dot lands, the paper unfolds, the corner flicks,
  and one eye winks.
- **Quiet hold:** a subtle breath and occasional blink while the host's local UI loads.
- **Into the app:** 12 frames at 60 fps, entered when `ready` is true.

The `Launch` view model has a boolean `ready` input (default false) and a numeric `cue`
output. The timeline writes cue 1 at landing, 2 at the corner flick, 3 at exit start,
and 4 at exit completion. Native observes those values; it does not schedule haptics
on wall-clock timers. On the CLI, a ready run reaches the exit cue at about 1.03 s.

```sh
rive artwork/brand/corner-hello --data=ready=true --advance=75 --data-dump=- --data-dump-every=1
rive artwork/brand/corner-hello --advance=33 --screenshot
```

The host chooses the display size and preserves the artboard's aspect ratio. It starts
local work under the greeting, waits for no network state, skips for Reduce Motion,
supports tap-to-dismiss, and cancels on inactivity. A five-second deadline also releases
the cover if rendering or setup stalls. Native launch failures never affect document
ownership, persistence or navigation.
