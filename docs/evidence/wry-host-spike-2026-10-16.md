# `tao` + `wry` host spike, macOS

A frozen snapshot, not release acceptance. Three rounds of a throwaway host built with
`tao` 0.37.1 and `wry` 0.57.0 (pinned exactly), on macOS 26.6 / arm64 with Rust 1.96.1,
about 1,000 lines in a Git-ignored scratch directory. No Windows or Linux code was built or
run. The question was whether a Rust host could replace or sit beside the Swift host. The
proposal this informs is [Other platforms](../ideas.md#other-platforms).

**Outcome:** not a replacement for the Swift host. A usable base for a Windows and Linux
host if the bridge admission and shape code are ours. Every result below is from this
environment only.

| Gate | Status | Evidence |
|---|---|---|
| G1 shaped window | Partial | A borderless transparent window shaped from a PNG's alpha runs, and cursor polling toggles click-through. Real click-through was not verified by the automated test. |
| G2 `slop:` protocol | Pass for serving, fail for sender identity in stock `wry` | Embedded assets, MIME, CSP and fetch work. The IPC handler receives only the frame URL. |
| G3 real owner | Pass (probe page) | `slop-engine --preview-owner` as the owner: config, a text edit, save, close, reopen in a new process. |
| G4 capture | Pass | PNG with alpha and a one-page PDF with extractable text, through WebKit's public snapshot and PDF calls on the raw `WKWebView`. Products use the Rust renderer for exports. |
| E1 native frame admission | Pass | An own `WKScriptMessageHandlerWithReply` accepts only the main frame at the expected origin. About 90 lines, public API only. |
| E2 hit test without polling | Partial | A native view whose `hitTest:` returns nil below alpha 26 plus a layer mask. In-process probes pass; click-through did not work in a hand test on macOS 26. |
| E4 / open a real slop | Pass | The production shell and a real document's `ui.js` and `ui.css`, served from the file, rendered and saved an edit that survived a reopen. |

## Findings

**Sender identity.** `wry`'s macOS handler reads only `frameInfo.request.URL` and drops
`isMainFrame` and `securityOrigin`. A sandboxed iframe at the same URL as the page reached
native IPC and reported the same sender URL as the main frame. The Swift host checks the
main frame and security origin natively. Registering our own handler beside `wry`'s and
checking both rejected all three iframe cases (ordinary, same-URL sandboxed, `srcdoc`)
and accepted the main frame; with the check disabled the same test failed. `wry`'s own
`ipc` handler stays reachable from every frame. A same-origin iframe calling the parent's
handler is attributed to the main frame, as in Swift. A cross-origin https iframe was not
tested.

**Transparency and the App Store.** `wry` sets `drawsBackground` through key-value coding on
the WebView configuration (`src/wkwebview/mod.rs` in 0.57.0), an undocumented key. Treat Mac
App Store distribution as unavailable for that path. Direct distribution is unaffected.

**Click-through.** `set_ignore_cursor_events` is all or nothing, so the spike polled the
global cursor and toggled it: steady sample gaps p95 9.15 ms, max 11.6 ms, with a race at
each boundary crossing. Missed clicks and flicker were not measured. The `hitTest` override
that the Swift host uses (`SlopWindow.swift`) was ported in 87 lines; a hand test on macOS 26
saw clicks on transparent pixels not reach the window behind. An automated click test could not
run, because the test process lacked Accessibility permission. Forum reports describe macOS
changing this behavior between releases.

**Real documents.** The preview-owner protocol serves attachments only, so opening a real
slop required the host to link `hitslop-core` and use its resource reader, the same calls
Swift's `SchemeHandler` makes through the FFI. With that, a copy of a 480 by 620 document
rendered correctly (checked from a WebKit snapshot) and a typed edit saved and reopened in a
new owner process. Not exercised: attachments, range requests, commands, widgets, window
resize and native window actions.

## Not covered

Windows, Linux, iOS and Android. WebView2 and WebKitGTK frame identity. Mixed-DPI
hit testing (`tao`'s macOS global cursor uses the primary monitor's scale). The Swift host's
own click-through on macOS 26. Catalog, Recents, menus, updater, telemetry, Quick Look and
widgets. Renderer-crash and failed-save recovery in the spike host.
