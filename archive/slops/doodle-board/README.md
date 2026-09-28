# Doodle Board

From the repository root:

```sh
bun slop dev examples/slops/doodle-board
bun slop build examples/slops/doodle-board
bun slop register examples/slops/doodle-board
```

Create a writable copy of the registered master to keep drawings. Preview data is
disposable. Drawing uses the MIT-licensed `perfect-freehand` package; its demo and
the reference checkout are not bundled. The host supplies the document engine.

Board shape and identifiable SVG strokes live in the document. Tool selection,
brush preferences, tray pinning, and sampled pointer input stay local. The Brush
popover affects new strokes only. Board presets and Fit board resize native
windows; browser previews retain their manually resizable frame. Scalar path previews participate in native flush;
pointer-up commits the completed stroke. There is no separate canvas save file.
