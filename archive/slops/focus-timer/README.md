# Pomodoro

A tactile tomato timer for focus sprints and short breaks. The live countdown is
transient; completed sessions and configured durations live in the document.

From the repository root:

```sh
bun slop dev examples/slops/focus-timer
bun slop build examples/slops/focus-timer
bun slop register examples/slops/focus-timer
```

The fixed circular window is 320 × 320. Recent focus counts include the last 12
saved sessions, including breaks. Build a writable copy before editing.
