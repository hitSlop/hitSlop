# Koi Pond

From the repository root:

```sh
bun slop dev examples/slops/koi-pond
bun slop build examples/slops/koi-pond
bun slop register examples/slops/koi-pond
```

A hidden WEBGL p5.js sketch paints the pond with p5.brush (LGPL-2.1 p5, MIT p5.brush; licenses in
`assets/`) only when the seed or size changes. A lightweight p5 2D sketch animates the koi,
ripples and pellets over the painted layers at 30fps, and pauses while the page is hidden.
Painting runs in ~8ms slices between animation frames, so the old pond keeps swimming until the
new one crossfades in; repeated Repaint requests coalesce to the latest seed.

The document stores the pond seed, the koi (name, pattern, size) and a feedings counter.
Fish positions, ripples, pellets and hover UI are local.
