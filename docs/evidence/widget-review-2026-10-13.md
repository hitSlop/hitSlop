# Widget review — 2026-10-13

The current branch was built as a development-signed macOS app, validated, and installed
at `/Applications/hitSlop.app`. The previous app was archived before replacement.

Before installation, PluginKit listed both the installed widget extension and an Xcode
development copy. System logs reported `Missing: com.hitslop.app:SlopWidgetIntent` when
configuration was attempted, although both bundles contained the intent metadata.
The development registrations were removed and the updated installed app registered.
PluginKit then listed exactly one widget extension, under `/Applications/hitSlop.app`.
This supports stale registration as a possible cause; updating and registration cleanup
were performed together, so the individual cause was not isolated.

The user confirmed after installation:

- Right-click → Edit opens the Document picker.
- Widgets stay behind another app after the editor is dismissed.
- Dragging moves the widget without launching hitSlop.

The widget retains standard click-to-open behavior. No gesture or window-layer workaround
was added. These observations qualify this installation, rather than every macOS version
or widget size.

Cache maintenance now updates a moved or renamed document's title, source location and
bookmark without recapturing its images or changing its selection ID or capture time.
Equivalent paths through linked parent folders do not consume reload requests. Regression
coverage includes unchanged sources, failed captures, deletion after a move, and refusing
copied or replaced files. The regressions failed before their fixes.

`bun run verify`, `bun run verify --native`, the signed Xcode build, strict bundle signature
validation and `scripts/release/artifact.ts` passed. Artifact validation checked widget
intent metadata in both bundles, live document commands, packaged templates, matching
shells, and editing/export without an evaluator override.
