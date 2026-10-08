# Pre-launch simplification — 2026-10-04

This historical summary retains the ownership decisions and integration findings from
an unsigned local build. It is not current release acceptance. The
[architecture](../architecture.md) and [engineering contract](../engineering-contract.md)
define the current system; the [roadmap](../roadmap.md) tracks remaining work.

## Ownership decisions

- One Rust owner admits edits, publishes state, schedules persistence, handles discard
  and close, and routes socket commands. Swift owns the window, page lifecycle, native
  services and ordered delivery. Closed data edits do not run authored JavaScript.
- Theme overrides share Loro storage, sequence, publications, undo and saving with data.
  A color-panel gesture forms one undo step; independent agent edits remain separate.
- Exports, previews and icons render a saved copy. Only draining, saving and copying
  hold an open editor's capture barrier; rendering can continue after the editor closes.
- Loro's merging and convergence semantics remain. Future sync needs transport,
  authentication, retention and remote-edit undo policies, rather than another engine.

## Integration findings

- Socket replies must survive peer closure. Nonblocking I/O with bounded polling covers
  buffered replies after close, stalled reads and backpressured writes.
- An uncertain command result must not claim that the edit was never applied.
  `unknown_outcome` distinguishes an unconfirmed final state; transport loss alone does
  not establish acceptance.
- A queued export must not capture a replacement owner after discard/reopen. The live
  request's epoch is checked across asynchronous flush/copy; an acquired copy can finish
  rendering independently.

The retained [synthetic theme gesture measurements](theme-drag-2026-10-04.json) cover
60 accepted changes and one Undo restoring the starting palette at 10 and 1,000 rows.
They do not establish physical color-picker or paint latency. System IME, physical picker
interaction and quarantine behavior require their own checks; this report does not
establish them.
