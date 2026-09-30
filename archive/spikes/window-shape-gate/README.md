# Native vector interaction gate

This diagnostic opens two AppKit windows in **separate processes**. The foreground
uses a clear borderless window, a layer mask with an offset even-odd hole, and geometric
view hit testing, matching the proposed native approach. It posts real WindowServer
mouse events: first into the hole, then onto the visible rim, then into the resized
hole. It does not substitute `sendEvent` or a `hitTest` assertion for OS delivery.

```sh
swiftc archive/spikes/window-shape-gate/main.swift -o /tmp/hitslop-shape-gate
/tmp/hitslop-shape-gate /tmp/hitslop-shape-gate-receiver.txt
```

Expected: the separate receiver gets the two hole clicks; only the rim increments
the foreground count. The run takes six seconds and temporarily moves the pointer.
The process exits 0 only for the expected counts, 1 for failed delivery, and 2
when event posting is unavailable. This is a feasibility diagnostic, not a production interaction test or full shape
implementation. Visual shadow and PNG checks follow only after this gate passes.

On 2026-09-30 this machine returned:

```text
BLOCKED: WindowServer event posting is not authorized; no click-through result.
```

The executable exited 2 before launching either test window or posting input.
macOS event-posting authorization is required to obtain a result. The terminal
sandbox permission granted to run the executable does not grant macOS Accessibility
access. No conclusion about hole behavior can be drawn from that automated run.
The later manual run below passed the feasibility gate; automated posting remains unauthorized.


## Manual mode (no event-posting authorization needed)

```sh
/tmp/hitslop-shape-gate --manual /tmp/hitslop-shape-lab-gate
```

Click the blue hole, orange rim and blue outside area separately. Press **R** while
orange is active to resize, repeat, and press **Q** to close both processes. A ten-minute
timeout also closes the diagnostic. Receiver activation can cover the foreground;
manual mode restores foreground ordering after the receiver records a click.

Each process writes its own `.foreground` / `.receiver` count and `.events` log under
the supplied prefix. These are observations, never an automatic pass in manual mode.
The automated mode now checks outside-outline delivery too, using separate result files.

On 2026-09-30 the maintainer confirmed correct initial/resized-hole and rim delivery.
See [the evidence and its limits](../../docs/evidence/shape-lab-interaction-2026-09-30.md).
The production geometry implementation remains separate from this feasibility pass.
