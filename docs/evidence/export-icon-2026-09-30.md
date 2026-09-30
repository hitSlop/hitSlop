# Export/icon migration evidence — 2026-09-30

Measured on Apple M1 / 16 GiB, dirty working tree based on `4c12ea89db2c186e6157efde02bab3770bf4d5d1`.

The pre-migration captures are in `.hitslop/export-baseline`; post-migration captures are in `.hitslop/export-after`. All 17 template builds completed in 120.9s total (includes compilation, native startup, preview and icon capture; this is not an isolated capture-preparation measurement).

Decoded comparison: **17 previews, 16 existing icons, 17 PNG exports and all 17 PDF pages matched exactly**. Small Expenses gained a new icon, inspected at 512px. PDFs were rasterized at 72dpi with `pdftoppm -r 72 -png` before comparison. Compression and PDF metadata were not compared.

Reproduce PNG comparisons by compiling `scripts/compare-captures.swift` with `swiftc -O`, then passing the before/after template or capture directories. Rasterize PDFs into separate directories and compare those too. Save baselines before source changes; never rebuild them in place.

## Results

```text
MATCH alien-radio.slop/QuickLook/Icon.png 512x512 -> 512x512 channels=0
MATCH alien-radio.slop/QuickLook/Preview.png 1440x1120 -> 1440x1120 channels=0
MATCH alien-radio.slop/assets/alien-radio-chrome.png 720x560 -> 720x560 channels=0
MATCH alien-radio.slop/assets/window-mask.png 720x560 -> 720x560 channels=0
MATCH doodle-board.slop/QuickLook/Icon.png 512x512 -> 512x512 channels=0
MATCH doodle-board.slop/QuickLook/Preview.png 1488x1042 -> 1488x1042 channels=0
MATCH habit-heatmap.slop/QuickLook/Icon.png 512x512 -> 512x512 channels=0
MATCH habit-heatmap.slop/QuickLook/Preview.png 1240x2216 -> 1240x2216 channels=0
MATCH harada-method.slop/QuickLook/Icon.png 512x512 -> 512x512 channels=0
MATCH harada-method.slop/QuickLook/Preview.png 1600x1354 -> 1600x1354 channels=0
MATCH kanban-board.slop/QuickLook/Icon.png 512x512 -> 512x512 channels=0
MATCH kanban-board.slop/QuickLook/Preview.png 1760x894 -> 1760x894 channels=0
MATCH meeting-notes.slop/QuickLook/Icon.png 512x512 -> 512x512 channels=0
MATCH meeting-notes.slop/QuickLook/Preview.png 1360x1130 -> 1360x1130 channels=0
MATCH metronome-tapper.slop/QuickLook/Icon.png 512x512 -> 512x512 channels=0
MATCH metronome-tapper.slop/QuickLook/Preview.png 860x678 -> 860x678 channels=0
MATCH morning-pages.slop/QuickLook/Icon.png 512x512 -> 512x512 channels=0
MATCH morning-pages.slop/QuickLook/Preview.png 1280x1352 -> 1280x1352 channels=0
MATCH pixel-art.slop/QuickLook/Icon.png 512x512 -> 512x512 channels=0
MATCH pixel-art.slop/QuickLook/Preview.png 920x976 -> 920x976 channels=0
MATCH pocket-sheet.slop/QuickLook/Icon.png 512x512 -> 512x512 channels=0
MATCH pocket-sheet.slop/QuickLook/Preview.png 1400x998 -> 1400x998 channels=0
MATCH quick-checklist.slop/QuickLook/Icon.png 512x512 -> 512x512 channels=0
MATCH quick-checklist.slop/QuickLook/Preview.png 960x1240 -> 960x1240 channels=0
MATCH reading-tracker.slop/QuickLook/Icon.png 512x512 -> 512x512 channels=0
MATCH reading-tracker.slop/QuickLook/Preview.png 1120x1234 -> 1120x1234 channels=0
MATCH recipe.slop/QuickLook/Icon.png 512x512 -> 512x512 channels=0
MATCH recipe.slop/QuickLook/Preview.png 1360x1156 -> 1360x1156 channels=0
MATCH slide-deck.slop/QuickLook/Icon.png 512x512 -> 512x512 channels=0
MATCH slide-deck.slop/QuickLook/Preview.png 1680x766 -> 1680x766 channels=0
MATCH small-expenses.slop/QuickLook/Preview.png 640x760 -> 640x760 channels=0
MATCH wordle.slop/QuickLook/Icon.png 512x512 -> 512x512 channels=0
MATCH wordle.slop/QuickLook/Preview.png 880x862 -> 880x862 channels=0
MATCH workout-planner.slop/QuickLook/Icon.png 512x512 -> 512x512 channels=0
MATCH workout-planner.slop/QuickLook/Preview.png 880x1862 -> 880x1862 channels=0
MATCH alien-radio.png 1440x1120 -> 1440x1120 channels=0
MATCH doodle-board.png 1488x1042 -> 1488x1042 channels=0
MATCH habit-heatmap.png 1240x2216 -> 1240x2216 channels=0
MATCH harada-method.png 1600x1354 -> 1600x1354 channels=0
MATCH kanban-board.png 1760x894 -> 1760x894 channels=0
MATCH meeting-notes.png 1360x1130 -> 1360x1130 channels=0
MATCH metronome-tapper.png 860x678 -> 860x678 channels=0
MATCH morning-pages.png 1280x1352 -> 1280x1352 channels=0
MATCH pixel-art.png 920x976 -> 920x976 channels=0
MATCH pocket-sheet.png 1400x998 -> 1400x998 channels=0
MATCH quick-checklist.png 960x1240 -> 960x1240 channels=0
MATCH reading-tracker.png 1120x1234 -> 1120x1234 channels=0
MATCH recipe.png 1360x1156 -> 1360x1156 channels=0
MATCH slide-deck.png 1680x766 -> 1680x766 channels=0
MATCH small-expenses.png 1280x1520 -> 1280x1520 channels=0
MATCH wordle.png 880x862 -> 880x862 channels=0
MATCH workout-planner.png 880x1862 -> 880x1862 channels=0
MATCH alien-radio-1.png 720x560 -> 720x560 channels=0
MATCH doodle-board-1.png 744x521 -> 744x521 channels=0
MATCH habit-heatmap-1.png 620x1108 -> 620x1108 channels=0
MATCH harada-method-1.png 800x677 -> 800x677 channels=0
MATCH kanban-board-1.png 880x447 -> 880x447 channels=0
MATCH meeting-notes-1.png 680x565 -> 680x565 channels=0
MATCH metronome-tapper-1.png 430x339 -> 430x339 channels=0
MATCH morning-pages-1.png 640x676 -> 640x676 channels=0
MATCH pixel-art-1.png 460x488 -> 460x488 channels=0
MATCH pocket-sheet-1.png 700x499 -> 700x499 channels=0
MATCH quick-checklist-1.png 480x620 -> 480x620 channels=0
MATCH reading-tracker-1.png 560x617 -> 560x617 channels=0
MATCH recipe-1.png 680x578 -> 680x578 channels=0
MATCH slide-deck-1.png 840x383 -> 840x383 channels=0
MATCH small-expenses-1.png 640x760 -> 640x760 channels=0
MATCH wordle-1.png 440x431 -> 440x431 channels=0
MATCH workout-planner-1.png 440x931 -> 440x931 channels=0
```

The new API also passed native shared-document, mode-propagation and failed-icon/master-preservation cases. Selected-view capture still uses shared transient state. Native tests also cover empty and selected Filed views, full-length PDF content and editor restoration. Pixel parity here covers each template’s initial data. [Capture timing](preview-capture-2026-09-30.json) records 1k/5k-row warm samples without truncating rows.

After switching compilers, all 17 icons and 15 previews matched the prior pixels exactly. Habit Heatmap was rendered across midnight, so its date-dependent calendar changed. Metronome differed in 91 color channels, each by one 8-bit level; dimensions and artwork were unchanged. These compiler comparisons are separate from the exact API-extraction parity above.
