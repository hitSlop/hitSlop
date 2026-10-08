# Powerful slops and the page policy (CSP)

Status: 2026-10-03. Probe done. WebAssembly is allowed for every slop and byte ranges are fixed (see Decisions). The worklet build step for Loop Lab, plain-language policy errors and one generated policy aren't started. Split out of `archive/docs/SingleFileSlop.md`: the
container doesn't change the policy, and these changes can land before or after it.

## Today

The app's policy (`apps/apple/Packages/HitSlopApple/Sources/HitSlopDocument/SchemeHandler.swift:28`):

```text
default-src 'none'; script-src slop:; connect-src slop: https: blob:; media-src slop: https: blob:;
frame-src https:; style-src slop: 'unsafe-inline'; img-src slop: data: https: blob:; font-src slop: data:
```

- **Workers and AudioWorklets.** With no `worker-src`, workers fall back to `script-src slop:`,
  and worklet modules are governed by `script-src`.
  - A worker or worklet file in `assets/` is allowed today.
  - A worker or worklet built from a `blob:` or `data:` string is blocked. That's how Tone's clock
    and Strudel's/superdough's published worklets are packaged, which cost Loop Lab its
    distortion, bit-crush and supersaw.
- **WebAssembly.** `script-src` has no `'wasm-unsafe-eval'`, so the app refuses to compile
  WebAssembly (confirmed by the probe below).
- **Remote code.** No `eval` and no remote scripts: "No executable code is downloaded"
  (`docs/reference/runtime.md:60`).
- **Two policies that already differ.** `packages/cli/src/dev.ts:104` holds its own copy for the
  preview:

  | | `slop dev` preview | App |
  |---|---|---|
  | `'wasm-unsafe-eval'` | allowed (its WASM core needs it) | not allowed |
  | `data:` fonts | not allowed | allowed |

  So a slop using WebAssembly works in `slop dev` and fails in the app.
- **Missing content types.** The scheme handler's type table has no `wasm`, `mjs`, `wav`, `ogg`,
  `webm` or `otf`. Unknown extensions are served as `application/octet-stream`, which breaks
  module workers loaded from `.mjs` and `WebAssembly.instantiateStreaming`. The single-file plan
  fixes this with a Rust-owned table and refuses nothing new.

## Decisions (2026-10-03)

- **WebAssembly is allowed for every slop** (user decision). It turned out to be a regression
  from the host-owned reset, not a new allowance:
  - the released 1.0.7 policy is `script-src slop: 'wasm-unsafe-eval'`, because Loro then ran
    in the page;
  - `docs/reference/runtime.md` already permitted WebAssembly;
  - the reset dropped it, so Soma Amp's MilkDrop (`onlyUseWASM: true`, no JavaScript
    fallback) failed to compile any preset, while `slop dev` still allowed it.

  `SchemeHandler` restores `'wasm-unsafe-eval'` and serves `.wasm` as `application/wasm`.
  Inline, `blob:` and `data:` JavaScript stays refused.
- **Byte ranges are fixed:** `Content-Length` on every response, and 206 for single ranges.
- **Tests:** `PagePolicyProbeTests` now asserts WebAssembly runs and packaged media seeks (both
  failed first) and that inline scripts stay refused. Soma Amp's presets compile (2 modules, no
  errors).
- WebKit fires no `securitypolicyviolation` for a refused WebAssembly compile. That's moot now,
  but plain-language errors must not rely on that event alone.

## Probe results (2026-10-03)

`PagePolicyProbeTests` opens a real document window and records what the page can run. The
evidence is in `docs/evidence/policy-probe-2026-10-03.json`.

| | Today | With the variant handler |
|---|---|---|
| Worker, module worker (`.js`, `.mjs`), AudioWorklet from a package file | run | run |
| `blob:` / `data:` workers and worklets | refused, with a `securitypolicyviolation` event | refused |
| WebAssembly (`instantiate`, `Module`, streaming) | refused by CSP; streaming also fails on the type | all run (`'wasm-unsafe-eval'` + `application/wasm`) |
| `crossOriginIsolated` / `SharedArrayBuffer` | false / undefined | still false / undefined with COOP, COEP and CORP |
| `<audio>` playing a package file | **fails** (error 4): WebKit asks for byte ranges, and the handler sends whole files | plays and seeks with 206 responses |
| `<audio>` from a `blob:` URL of fetched bytes, `decodeAudioData` | work | work |

What this settles:
- **Worklets need no policy change.** Shipping them as files is enough, so the Loop Lab work is
  purely a build step.
- **WebAssembly is a pure policy decision.** The mechanism works once allowed. The second
  write-up's claim that same-scheme streaming needs no `'wasm-unsafe-eval'` is wrong.
- **`SharedArrayBuffer` is off the table** for `slop:` pages. Don't promise threaded
  WebAssembly or shared-memory audio.
- **Byte ranges are a bug fix, not an option.** Packaged `<audio>` and `<video>` don't work
  today. No shipped slop does this (Alien Radio streams over HTTPS), so it was latent.
- **Tests that probe media need a window:** WebKit starts media only for a page in one.

The test asserts the policy facts we rely on: package workers and worklets run; `blob:`/`data:`
code and WebAssembly are refused. It doesn't assert WebKit's isolation limit. Media gets its
assertion with the range fix, which it fails until then.

## Not exploring

| Idea | Why not |
|---|---|
| JS wrappers around `WebAssembly.compile/instantiate` limited to `slop:` bytes or hashes | Not a boundary. Workers and fresh iframes get untouched globals, and the policy would still have to allow compiling. |
| A native `compileWasm(assetPath)` bridge | WebKit has no API to give a page a module compiled by the host. |
| CSP hash sources for WebAssembly | Not a CSP feature; `'wasm-unsafe-eval'` is all-or-nothing. |
| Extism or Wasmtime in the host | Moves authored code from WebKit's sandboxed content process into the process that owns documents and storage, against "closed edits never run authored code" and "no second engine". It doesn't serve real-time audio. |
| Extism in the page | Still needs `'wasm-unsafe-eval'`. |
| COOP/COEP headers or `coi-serviceworker` | The probe shows WebKit doesn't isolate `slop:` pages even with the headers. |
| memmap2 range slicing | SQLite blobs aren't contiguous, and untrusted files run with `mmap_size=0`. Use incremental blob reads. |
| oxc or swc rewriting in Rust | Rolldown already runs oxc; a build plugin is enough. |
| wasmparser, walrus, Javy, Component Model | Only relevant once WebAssembly is allowed. |
| SES/Compartments, ShadowRealm, QuickJS | SES needs `eval`, ShadowRealm isn't in WebKit, QuickJS needs WebAssembly or a host engine. A slop can ship a JS interpreter today. |
| `<iframe sandbox>` around the app | Each document already has its own WebView and content process. |

**Later, as a product decision:** per-slop network declarations that narrow `connect-src`,
`img-src`, `media-src` and `frame-src`. Pair it with permissions bound to the code hash.

## Proposals

1. **One policy, owned by `packages/schema`.** Generate it for Swift and the dev server, the way
   the limits are generated today, and state `worker-src slop:` explicitly. To make the preview
   match the app, either:
   - run the preview's WASM core in a worker served with its own policy, so the page doesn't
     need `'wasm-unsafe-eval'`; or
   - document the difference and have `slop build` warn when an app instantiates WebAssembly.
2. **The platform bundles workers and worklets, not authors.** Teach `slop build` to emit worker
   and worklet entry points as files in `assets`, using Vite's
   `new Worker(new URL('./w.js', import.meta.url), { type: 'module' })`, `?worker` and `?url`.
   `addModule` and `new Worker` then get `slop://app/assets/...` URLs.
   - For libraries that publish inline `data:` worklets (superdough), build from their source.
   - Don't allow `blob:` or `data:` workers: that would let a slop run fetched text as code.
3. **WebAssembly (decided above: allowed).** The analysis that led there: `'wasm-unsafe-eval'` would let a slop compile bytes
   fetched over `https:` (allowed by `connect-src`). The rule "everything the engine executes
   ships in the file" would then become "all JavaScript ships in the file."
   - The rule is already about what runs at native speed, not about behavior: a slop can ship an
     interpreter and fetch programs as data. Loop Lab evaluates typed patterns.
   - Options:
     - keep WebAssembly blocked;
     - allow it everywhere and restate the rule;
     - allow it per slop as a permission granted to the app's code hash (point 4).
   - Recommendation: keep it blocked until a real slop needs it, then decide with that slop in
     hand.
4. **Permissions bound to code (later).** In a single-file slop, the `app` row plus `assets` is
   all the code there is, so a hash of that canonical content identifies it. A permission
   (microphone, camera, MIDI, a WebAssembly opt-in) can be granted to *this* code, and a Remix or
   upgrade asks again.
5. **No `SharedArrayBuffer`.** The probe shows WKWebView doesn't isolate `slop:` pages, even with
   COOP, COEP and CORP headers. Don't offer threaded WebAssembly or shared-memory audio.
6. **Byte ranges (a bug fix).** Packaged `<audio>`/`<video>` fails today, because WebKit needs
   206 responses. `SchemeHandler` answers `Range` with 206, `Content-Range`, `Content-Length`
   and `Accept-Ranges`, and adds `Content-Length` to every response. Read file offsets today,
   and SQLite incremental blob reads after the single-file change. The probe test gains a
   media assertion that fails before the fix.

## Verification

- `PagePolicyProbeTests` (done): package workers and worklets run; `blob:`/`data:` code and
  WebAssembly are refused. After the range fix, packaged media plays and seeks.
- `slop dev` and the app give the same results for the same test app, after proposal 1.
