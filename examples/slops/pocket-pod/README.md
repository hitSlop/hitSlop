# Pocket Pod

A pocket click-wheel player. Paste a YouTube link anywhere on it and the video joins your library; spin the wheel to browse in Cover Flow, press the center to play, and cover the case in stickers.

From the repository root:

```sh
bun slop dev examples/slops/pocket-pod
bun slop build examples/slops/pocket-pod
bun slop register examples/slops/pocket-pod
```

Playback uses `@hitslop/document/embed`, which embeds YouTube through a relay page on hitslop.com, so it needs the network. Colors come from the theme tokens in `theme.ts`: `slop theme set case "#2ea06b"` makes it green. The name avoids Apple's trademarks on purpose.

Keys: ↑/↓ scroll, ←/→ previous and next, Return selects, Space plays or pauses, Esc goes back, and ⌘V adds a link. Hold the center button on a video to remove it.
