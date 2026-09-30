# Alien Radio

A PNG-skinned SomaFM receiver. The checked-in SVG is authoring artwork; `artwork/render.sh` generates the chrome and the window mask.

From the repository root:

```sh
bun slop dev examples/slops/alien-radio
bun slop build examples/slops/alien-radio
bun slop register examples/slops/alien-radio
```

Create a writable copy of the registered template before editing. Playback starts only after play. Streams come from SomaFM; the receiver does not proxy or record them.
