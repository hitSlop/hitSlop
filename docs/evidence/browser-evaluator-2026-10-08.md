# Browser command evaluator qualification

The current QuickJS command path is feasible in a disposable browser WASM worker.
This qualifies an evaluator route, not a browser document host. No `slop open --browser`
command or durable browser owner is implemented by this experiment.

## Measured

The local harness in `spikes/browser-evaluator` compiles the same pinned rquickjs 0.14.0
and includes the current ABI 1 command prelude verbatim. A fresh worker and QuickJS realm
are created for each request and terminated after its reply, failure or deadline.
The only WASM import is a clock function; authored QuickJS code has no browser, network,
filesystem or document-owner bindings. The worker's trusted loader fetches the module.

[Recorded results](browser-evaluator-2026-10-08.json): 30 cases passed across installed
Chrome 154.0.8037.98 and Playwright WebKit 27.2 on macOS. WebKit is not a Safari product
qualification. No mobile browser was tested.

- Stored Hourglass and Quick Checklist programs returned exactly the native evaluator's
  results and intents for the corpus scenarios, using the same descriptor, state, time
  and random seed. Applying those intents through the native Rust owner to temporary
  SQLite copies produced the corpus's expected saved values.
- Authored refusal preserved its message and `refused` flag. A command that staged a
  write and then threw returned no intents. Promise-returning commands were refused.
- Ambient `Date.now()` was refused. Nine browser/Node capabilities were undefined.
  Repeated calls had fresh globals.
- The 64 MiB input and 4 MiB output limits refused oversized requests/replies.
  A future runtime ABI was refused before evaluation.
- The evaluator uses a 256 MiB QuickJS heap cap and a 2-second interrupt deadline;
  the parent terminates a worker after 3 seconds. A 300 MiB typed-array allocation
  failed before the watchdog. An allocation loop required the watchdog in this run.
- Recursive code can exhaust the browser's WASM call stack before QuickJS returns its
  own stack error. Both engines reported a worker fault with no intents; the instance
  was discarded, and the following request succeeded in a fresh worker. Production
  routing must preserve this failure path, including traps and worker termination.

The module is 832,636 bytes (346,621 bytes gzip). Individual fixture evaluations took
33–45 ms with 1.5 MiB of WASM linear memory after evaluation. These are single samples,
exclude worker startup/fetch/compilation and are not end-to-end performance claims.
Native compilation was running concurrently. Digests of the module and prelude are
included in the JSON report.

## Reproduce the local spike

```sh
CC_wasm32_unknown_unknown=/opt/homebrew/opt/llvm/bin/clang \
AR_wasm32_unknown_unknown=/opt/homebrew/opt/llvm/bin/llvm-ar \
cargo build --offline --release --target wasm32-unknown-unknown \
  --manifest-path spikes/browser-evaluator/Cargo.toml
bun spikes/browser-evaluator/run.ts
```

This uses the existing native build and dev corpus, installed Chrome, and Playwright's
WebKit installation. The spike remains local under the repository's `spikes/` policy.

## Remaining integration

Move evaluator limits, ABI dispatch and evaluation into shared Rust code before shipping
a browser adapter; the probe must not become a second maintained command implementation.
Connect failures to the existing Rust command outcome handling, and validate intents in
the browser's real owner. Add this qualification to the normal runner when that adapter
exists. Test Safari itself, the full corpus and supported resource limits.

The current `hitslop-core --features storage` build still fails for wasm32: its file,
owner and command modules are native-only, and storage dependencies and scheduling need
separation. The owner uses threads, channels and native clocks. The previous SQLite/OPFS
experiment does not prove this owner works in a browser. The next implementation milestone
is the shared owner driver and portable store, followed by actual app edits, durable
reload, focused-text download and native reopen. Hosted uploads and share URLs follow
that round trip.
