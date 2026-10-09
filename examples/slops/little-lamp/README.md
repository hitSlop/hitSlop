# Little Lamp

A transparent desktop companion, authored in Rive. Tap the lamp (or focus it and
press Space/Enter) to wake it or send it to sleep. While awake it follows the
pointer, breathes, and blinks. There is no card or separate switch.

## What Rive owns

The .riv contains a two-bone inverse-kinematics arm, a pointer-driven head target,
four state-machine states, and coordinated vector animation for the shade, eyes,
and light. Wake and sleep clips include anticipation, squash/stretch and damped
overshoot. Interrupting a clip blends into the opposite action.

Svelte supplies an accessible, visually unstyled tap target and normalized
pointer data. It does not animate the character. Only the on/off boolean is saved
through the hitSlop document SDK; pointer position and playback stay transient.

Reduced motion disables tracking and idle playback and uses immediate poses.
Hidden windows stop rendering. The sleeping lamp stops after its nod finishes.
The native window has a static path around the character's motion envelope;
transparency within that envelope is visual, not per-pixel click-through.

## Run and build

From the repository root:

```sh
bun slop dev examples/slops/little-lamp
bun slop build examples/slops/little-lamp --artwork native
bun slop register examples/slops/little-lamp
```

The example is active but not selected in bundled.json. Existing documents retain
their embedded app: create a fresh document from the updated local template.

## Rebuild the scene

Authored with **Rive CLI 1.5.0**, played by **@rive-app/canvas 2.44.0**.

```sh
bun examples/slops/little-lamp/regenerate.ts
```

Set RIVE_BIN to an absolute executable path if needed. The command verifies and
inspects rive/scene.rml, then builds assets/little-lamp.riv. No account, scripts,
publishing or signing are required. Install the CLI using
[Rive's instructions](https://rive.app/docs/cli/getting-started).

To inspect a mid-bounce frame or a pointer pose:

```sh
rive examples/slops/little-lamp/rive --screenshot=/tmp/lamp-bounce.png --advance=1 --data=on=true --advance=350ms
rive examples/slops/little-lamp/rive --screenshot=/tmp/lamp-look.png --data=on=true --data=animate=false --data=lookX=18 --data=tilt=0.14 --advance=1s
```

The Lamp artboard's Power state machine reads the Lamp view model:

| Property | Purpose |
| --- | --- |
| on | The saved document boolean |
| animate | Whether a change should play wake/sleep or jump to its resting pose |
| lookX / lookY | Transient head offsets; Rive solves the arm |
| tilt | Transient head rotation |

Export and icon components use the same Rive renderer in still mode, reading
saved data. The host capture hook waits for the scene to load and draw. PNGs retain
transparency; no dark preview-background screenshots are shipped as artwork.

The JS runtime and matching WASM are packaged together; CDN fallback and hosted
asset loading are disabled. Keep the runtime dependency exactly pinned.

## Checks

```sh
bun slop check examples/slops/little-lamp
bun run verify
```

Manually review tap/keyboard interaction, pointer tracking, transparency, undo,
resize and reduced motion. Check a fresh packed document in the Mac app, including
save/reopen and native PNG/PDF capture. These are review steps, not a dedicated
automated test suite for the lamp.

The original artwork is covered by the repository's MIT license. Rive's MIT
notice is in assets/rive-license.txt and retained in the portable page.
