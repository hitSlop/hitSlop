# v1.0.0 boundary and stress review

Review date: 2026-10-16. This PR records findings and runnable experiments; it does
not fix production behavior.

The architecture is a sound foundation for continuing v1. The single Rust owner,
shared CLI edit path, immutable embedded apps, independent compatibility markers and
original-writer corpus are the right boundaries. Keep them. Two reproduced defects
need a focused hardening patch: untrusted SQLite can execute before its layout is
validated, and an unresolved text draft can be discarded on control teardown.

**Recommendation: fix these in v1.0.1, qualify it on macOS, and preserve the published
v1.0.0 baseline.** A host rewrite or resetting v1 would add risk without addressing
either failure. Broad web access and the initial camera permission policy are
intentional product choices for this review, rather than defects to block launch on.

## Scope and identity

Source reviewed: `fce09d7661e9759e689b76d7788e37d8ce203c1f` on `master`.
Release tested: `v1.0.0`, commit `0d2103991a93cf06377b9a4583964c4ddd4fa05c`.
The runtime, SDK and Apple source under review match that release; the subsequent
changes concern compatibility and release tooling.

| Artifact | SHA-256 / identity |
|---|---|
| `hitslop-1.0.0.tgz` | `38c2bb3efb551526307971e8f03f76132b959018a1cf2a9ffe30fa19babd71c4` |
| Released Linux x64 `slop-engine` | `439b91a0fbed620ab4914b98034cd48a22328dfd6d1a2239c27226712fdbe44c` |
| Core build ID | `309c082728ce5de9` |
| `compat-corpus-1.0.0.tar.gz` | `a73c01b715c640730a8e901284cbe8b91178cfaf6ff494e13f75f93c297bd079` |

Experiments used the released Linux x64 binary, original release corpus, disposable
copies, seed `20261016`, Python 3 and Bun 1.4.2. The draft reproduction runs the actual
page-shell module with a deliberately failing transport. The preview protocol runs
the production Rust owner/store, but has its own admission cap. It does not exercise
the Swift/WebKit bridge. Machine-readable observations are in
[the evidence record](v1-stress-review-2026-10-16.json).

## P1 — a 4 KiB file can stall validation indefinitely

**Confirmed in the released engine.** The marker reader in
[`file/check.rs`](../../crates/hitslop-core/src/file/check.rs) executes
`SELECT package_format, runtime_abi FROM app WHERE id=1` before `layout()` compares
the schema. A file can supply an `app` view instead of the expected table:

```sql
PRAGMA application_id=1213418576;
PRAGMA user_version=1;
CREATE VIEW app AS
WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n)
SELECT 1 id, 1 package_format, (SELECT max(x) FROM n) runtime_abi;
```

Inspecting that 4,096-byte file produces no reply before the experiment kills the
process at two seconds. A normal wrong table is refused in milliseconds. Repeated
trials reproduce the stall and leave the input unchanged. The recursive aggregate
has no practical completion bound. `busy_timeout` only bounds lock waits; it does
not bound SQL execution. Defensive mode and `trusted_schema=OFF` do not stop it.

The native impact is inferred from the shared reader and
[`SlopPreparation.documents`](../../apps/apple/Packages/HitSlopApple/Sources/HitSlopCore/SlopPreparation.swift):
document preparation uses a serial queue, and cancellation is checked before work
starts. One hostile open can therefore occupy that queue and delay later opens.
This is an availability defect; no code execution, data corruption or native UI
freeze was demonstrated.

Fix at the file boundary:

- Preserve the early `requires_update` refusal for a newer storage version.
- Inspect bounded schema metadata before querying file-supplied objects. Require
  `app` to be the appropriate ordinary table with ordinary marker columns before
  reading its markers. Do not move an exact storage-1 comparison ahead of every
  marker and accidentally change future-format refusal behavior.
- Apply an interruptible execution/instruction budget to untrusted preflight and
  catalog reads, including schema enumeration. Bound metadata allocation too.
- Extend the existing file-boundary tests with this minimal file, a bounded refusal,
  unchanged input bytes and a subsequent valid open. Replay the frozen corpus.

Reproduce with `stress-release.py --sections files` below. The hang is recorded as
a failure observation, not converted into a passing conformance expectation.

## P1 — detaching a text control can silently lose its unresolved draft

**Confirmed in the production page shell with fault injection.** In
[`owner/document.ts`](../../packages/hitslop/src/shell/owner/document.ts),
`bindText().destroy()` removes the binding's drain in `finally`, even if its final
commit has an unresolved outcome. Later `prepareClose()` can succeed using the old
owner state. The remaining error report contains the connection error, not the
draft or enough information to recover it.

[`owner.test.ts`](../../packages/hitslop/tests/shell/owner.test.ts) already contains
the test “a destroyed text binding with an unresolvable draft does not block close.”
It intentionally verifies the successful close and unchanged old value. That fixes
a stuck-close problem, but does not provide a recovery path for the person's text.
Unmounting can happen during conditional rendering, navigation or list removal;
it is not limited to closing a window.

The included [reproduction](../../scripts/dev/repro-detached-draft.ts) sets the field
to `important unsaved draft`, makes apply return `unknown_outcome`, destroys the
binding, and closes. Result: two send attempts, one successful flush, close succeeds,
saved value remains `before`, and no reported error contains the draft. The transport
is mocked; this establishes the shell's behavior, not the incidence of native
connection failures.

Fix by retaining unresolved text and its location in a session-owned recovery
collection after the control disappears. Give the person copy/recover/discard
actions. If closing is allowed, recovery must survive that close. Keep this a draft
recovery mechanism, not a second persistent document model. Do not blindly replay an
unknown mutation, and do not restore an impossible-to-resolve permanent close barrier.
Extend the existing teardown test to assert an observable, usable recovery path.

The shell is host-supplied, so this can be corrected for existing embedded apps
without rebuilding their bundles or raising their runtime ABI.

## P2 — bound admission to the native owner, then qualify overload

**Source-reviewed risk; native memory exhaustion was not reproduced.**
[`owner/actor/mod.rs`](../../crates/hitslop-core/src/owner/actor/mod.rs) creates an
unbounded owner inbox and
[`Owner::enqueue`](../../crates/hitslop-core/src/owner/mod.rs) sends directly into it.
A per-request size limit does not bound aggregate queued memory. The current page
payload bound is 4 MiB; attachment and socket paths have separate bounds. Push
delivery, held barrier work, socket clients and preview requests have useful caps,
but those are different queues from the native inbound owner mailbox.

Put nonblocking count and byte admission budgets at the shared owner boundary, with
per-document and process-wide accounting where resources are shared. Refuse excess
work definitely before acceptance. Reserve capacity for completions, save, close and
recovery. A blocking bounded `send` on the main actor would create another hang.
The persistence channel is also structurally unbounded, although the one-save-in-flight
policy limits its producer; an unbounded declaration alone does not establish a leak.

Qualify a raw page-message flood, large attachment work, multiple documents and
parallel captures on macOS. Check memory, responsiveness, truthful outcomes and
successful close after overload. The preview's 64-request admission rule cannot
stand in for that test.

## Recovery and performance follow-ups

CLI `get` and `describe` flush before returning state in
[`command/native.rs`](../../crates/hitslop-core/src/command/native.rs). Copy and
export also require a save. That preserves their current saved-state promise, but
leaves no obvious CLI rescue route for unsaved owner state if persistence is failing
and the renderer is unavailable. Consider an explicit diagnostic/recovery export
with clearly stated unsaved semantics, rather than changing ordinary `get` silently.
The current tests did recover from busy, moved and full saves.

The adversarial 12,000-edit soak repeatedly replaces a whole 1 KiB text value with
unrelated random text. It completed in 613.264 s, p50 50.989 ms and p99 52.741 ms,
with the final saved value verified after reopening. This deliberately stresses the
50 ms text-diff budget in `text.rs`; it is not a normal keystroke benchmark. Live
history grew from about 0.34 MB to 12.2 MB during the sampled interval. Long-session
history growth is already a documented limitation. Keep a soak budget as a follow-up
before adding collaboration or many simultaneously active documents.

At the storage boundary, adding the 32nd MiB of high-entropy row text produced
`save_failed: document is full`. Deleting that last row and retrying succeeded.
Peak sampled owner RSS was 214,828 KiB in this workload: a 32 MiB persisted-data
bound is not a 32 MiB process-memory guarantee. An earlier soak RSS sampler selected
the wrong PID namespace; those readings were discarded, not used as evidence of
constant memory. The included sampler fixes that lookup.

## What was actually exercised

| Experiment | Observation |
|---|---|
| Original release corpus | 77 manifest hashes checked; 11 templates created/read; four saved documents read, attachments checked, scenario edits saved and reopened |
| Deterministic file/snapshot mutation | 512 cases: 457 refused, 55 accepted, no crashes, timeouts or reader changes to input bytes; acceptance can mean unused bytes were changed |
| Hostile layout and markers | Recursive-view stall above; ordinary wrong layout, empty history, damaged checkpoint and newer storage refused without writes |
| Invalid atomic batches | 96 cases across eight invalid-intent patterns, with a valid side effect before/after the invalid intent; all refused, no partial changes |
| Concurrent writers | 240 requests from 12 workers: 233 acknowledged, counter delta exactly 233; seven refused, no lost or duplicated acknowledged increments |
| Save failure/retry | Exclusive SQLite lock and moved file both report `save_failed`; restoring access and retrying retains accepted edits |
| Writer interruption | 40 large-input and 36 varied-delay small-input trials; every reopened document entirely old or new, no partial/unreadable state. Small trials include completed processes as well as SIGKILLs; this is not a physical-power-loss test |
| Path/publication safety | Linked leaf read and hard-link write refused; create/copy/PNG export refuse existing destinations and preserve the sentinel |
| Restricted evaluator | Host-I/O globals absent; infinite loop interrupted around 2 s; recursion, heap exhaustion and oversized output return bounded failures; prototype changes do not survive a fresh job |
| Captures | Four saved documents each exported as PNG and PDF; decoded PNG sizes correct, PDFs readable with CJK text extractable |
| Capacity | Full save refused, delete-and-retry succeeds; corrected RSS samples retained |
| Continuous owner | 12,000 randomized text replacements plus periodic saves; final value survives reopen |
| Detached draft | Actual shell module, failing transport: successful close with old value and no recoverable draft |

These experiments did not prove the absence of memory-safety vulnerabilities or
cover every authored program. Native WebKit/Swift lifecycle, renderer termination,
IME/focus, camera/speech, macOS permissions, Quick Look and mobile behavior were not
executed here. Browser installation failed because browser downloads were truncated.
The local Rust compiler exits with SIGBUS even for `rustc --version`, before project
code runs; full source-built verification was therefore unavailable. The released
binary was used rather than attributing that environment failure to hitSlop.

Local repository checks: `bun run verify types` passed; `bun run verify tooling`
passed (39 tests, 264 expectations); Python syntax and `git diff --check` passed.
`bun run verify` was attempted and stopped at the compiler identity check. A macOS
release candidate still needs the normal complete release qualification.

## Reproducing the experiments

Download the npm tarball and original corpus from the immutable
[v1.0.0 release](https://github.com/hitSlop/hitSlop/releases/tag/v1.0.0), verify the
hashes above, and extract them. Do not substitute newly rebuilt corpus fixtures.

```sh
python3 scripts/dev/stress-release.py \
  --engine /path/to/package/engine/linux-x64/slop-engine \
  --corpus /path/to/compat-corpus \
  --mutations 512 --soak-updates 12000 --output /tmp/hitslop-stress.json

bun scripts/dev/repro-detached-draft.ts
```

Use `--sections files` for the crafted-file repro or select comma-separated sections
to shorten a run. The complete soak takes roughly ten minutes on this review machine.
The scripts operate on disposable copies and an isolated registry. They are manual
audit tools, not a new per-example CI suite. Put fixed regressions into the existing
owning-boundary tests when implementing the production fixes.

## Release and simplification plan

1. Fix safe file preflight and detached-draft recovery as separate, reviewable changes.
2. Add owner admission bounds and run native overload qualification before a wider
   distribution push. At minimum, explicitly track it if it cannot join the first patch.
3. Replay the original v1.0.0 corpus through native open/render/edit/save/reopen,
   including both compacted and incremental history, captures and attachments.
4. Qualify failed saves, pending text and repeated open/close on macOS; run
   `bun run release:check` on the final clean candidate and publish **v1.0.1**.

Keep the existing release tag, assets, history pin and compiled app corpus immutable.
These repairs need no persisted-format or runtime-ABI bump if existing valid documents
remain accepted and public behavior remains compatible. If a fix must reject a formerly
accepted document, document the specific exception and a data-recovery path. Do not
tighten old acceptance as a shortcut. Later new functionality can use 1.1.x normally.

The useful simplification is to centralize boundary policy: safe preflight in the
file reader, admission in the owner, recoverable drafts in the shell, generated wires
between adapters. Preserve one semantic engine and one edit path. There is no evidence
here that replacing Loro, collapsing compatibility markers or rewriting the host would
improve launch stability.

## Rust UI / Wry assessment

[Wry](https://github.com/tauri-apps/wry) is a viable WebView adapter to investigate.
It uses platform WebViews: WebKit on Apple platforms, WebView2 on Windows and
WebKitGTK on Linux, with Android integration as well. Pairing it with an event loop
does not require adopting all of Tauri or rewriting the shell UI in egui. Rust would
still host HTML/Svelte content; Wry does not turn the entire host UI into Rust widgets.

| Option | Practical consequence | Recommendation |
|---|---|---|
| Swift host + shared Rust owner | Retains current window lifecycle and Apple integrations; other platforms need adapters | Keep for the hardening release |
| Wry host + existing HTML shell | Reuses the web UI and Rust semantics; platform WebView and native-service integration still need qualification | Build a small desktop portability spike |
| egui host + embedded WebViews | Potential shared catalog/toolbars; WebView composition, accessibility, IME and mobile integration still require work | Consider after the adapter spike, based on measured benefit |
| Full desktop/mobile rewrite now | Combines UI, platform services, security-boundary and lifecycle changes | Defer until the foundation is qualified |

Wry is not a drop-in replacement for the current security boundary. Its
[WebViewBuilder documentation](https://docs.rs/wry/latest/wry/struct.WebViewBuilder.html)
describes different custom-protocol origins on Windows/Android versus macOS/iOS/Linux;
on Linux/Android the IPC request URL does not identify the iframe URL. Windows also
does not honor the initialization-script main-frame-only flag the same way. Re-establish
sender, main-frame, document/view identity, navigation and CSP checks per platform.
Do not mechanically port a URL comparison and assume identical frame authentication.

Native capabilities remain accessible through platform-specific Swift/Objective-C,
Kotlin/JNI or plugin code. Tauri's
[mobile plugin model](https://tauri.app/develop/plugins/develop-mobile/) demonstrates
that bridge. What is lost in a wholesale rewrite is the current implementation and
its tested lifecycle, not the ability to call native APIs. Quick Look, sharing,
notifications, speech, menus, updater and window behavior remain platform work.

There is also Rust platform work beyond the UI: the current native registry and
socket modules import Unix APIs, file publication has macOS/Linux implementations,
and the restricted evaluator relies on a child process and platform sandbox. Windows
needs corresponding drivers; iOS/Android need an explicit evaluator/isolation design.
Wry's [mobile setup](https://github.com/tauri-apps/wry/blob/dev/MOBILE.md) still involves
Xcode/Android tooling. Five supported WebView targets do not make this entire runtime
ready on five platforms.

Start with a Linux Wry adapter, then Windows, keeping the Swift Mac host shipping.
The spike should open an untouched v1.0.0 document, authenticate the bridge, edit/save/
reopen, handle attachments, capture PNG/PDF, and survive renderer and save failures.
Use the same corpus and boundary tests. Decide whether to share catalog UI only after
that proves the actual platform cost; mobile is a separate qualification milestone.
