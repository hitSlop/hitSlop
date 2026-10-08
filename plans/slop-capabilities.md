# Slop capabilities: devices, speech, AI and integrations

Status: design direction, updated 2026-10-07. Not implemented. Camera and microphone
are the first implementation slice; the rest preserves the broader ideas. API examples
are proposed, not available today.

## What this enables

Slops should be able to use the device and services around them: record a voice note,
take a photo, transcribe an interview, generate flashcards, control music, or schedule a
reminder.

Use standard web APIs where they fit. Add typed host services where native frameworks,
credentials, or shared infrastructure make the capability more useful. Camera/mic is
the first useful milestone, not the boundary of the capabilities design.

Authors declare the capabilities their slop uses:

```ts
export default defineSlop({
  // Existing app fields…
  capabilities: ["microphone", "camera", "speech", "ai"],
});
```

Only implemented capabilities become accepted declaration values. Initially, declared
capabilities pass hitSlop's admission check without a separate hitSlop permission
prompt. OS/browser permission still applies. Once macOS allows hitSlop access, another
declaring slop can request capture without a per-slop prompt. Per-slop consent and
remembered grants remain future work.

## Camera and microphone

Use `getUserMedia`, `MediaRecorder`, Web Audio and canvas. Streams stay in the page;
there is no reason to send live audio/video through Rust or invent `ctx.camera` and
`ctx.microphone` wrappers.

```ts
const stream = await navigator.mediaDevices.getUserMedia({
  audio: true,
  video: true,
});
```

This opens up voice recorders, camera journals, instrument tuners, audio visualizers
and photo tools before any speech or AI provider exists. Completed photos and recordings
can become document attachments through the existing `attachments.import` API. Live
streams, playback position and recording buffers remain transient.

Authors should start devices from an explicit action, show capture state, handle normal
browser errors, and stop tracks when finished. The host admits capture only from the
interactive slop page, not embedded third-party frames or export/background renderers.

## Speech and AI

Speech turns recorded audio into text:

```ts
import { speech } from "hitslop/svelte";

const transcript = await speech.transcribe(recording, {
  locale: "en-US",
});
```

Start with recorded clips; live dictation could follow. Local recognition is attractive,
but a promise of on-device processing must mean no silent network fallback. An
unsupported device or language needs an understandable unavailable result.

AI supports both plain text and typed structured output:

```ts
import { s } from "hitslop";
import { ai } from "hitslop/svelte";
import doc from "./schema";

const summary = await ai.generate({
  prompt: transcript,
  instructions: "Summarize the main decisions.",
});

const { cards } = await ai.generate({
  prompt: `Create flashcards from: ${transcript}`,
  output: s.object({
    cards: s.list(s.object({
      front: s.string(),
      back: s.string(),
    })),
  }),
});

await doc.change(tx => {
  for (const card of cards) tx.fields.cards.insert(card);
});
```

The intended overloads remain:

```ts
generate(request: {
  prompt: string;
  instructions?: string;
}): Promise<string>;

generate<N extends Node>(request: {
  prompt: string;
  instructions?: string;
  output: N;
}): Promise<Input<N>>;
```

Scalar outputs such as `s.enum(["positive", "negative"])` should also be useful.
Generation returns a value; saving it remains an explicit document edit. A later
`into:` convenience could derive the shape from a destination handle and insert the
result, for example `ai.generate({ prompt, into: doc.fields.cards })`.

### Structured output belongs in the core

Keep the original `s.*` design. Authors already use this vocabulary and get an inferred
result type from `Input<N>`. Raw JSON Schema should not become a second author-facing
schema language or validator.

The intended flow is:

1. Rust checks the requested descriptor.
2. Rust projects a supported provider schema.
3. A provider generates against that projection.
4. Rust validates the returned value, including constraints the provider does not enforce.
5. Only a checked value reaches the author.

The earlier `output_schema(node)` and `check_output(node, json)` functions remain
useful sketches. JSON Schema here is an output for providers, just as it is a projection
for existing tool clients; it is not the stored document contract.

Useful mapping ideas to retain:

| Descriptor | Generation shape |
| --- | --- |
| `object` | Named properties and required fields |
| `list` | Array with a checked item shape |
| `string`, `text` | String |
| `number` | Number |
| `integer`, `counter` | Integer |
| `boolean` | Boolean |
| `enum` | One of the declared strings |
| `optional` | An optional property where the provider supports it |
| `record` | Initially exclude; dynamic keys need a separate decision |

Share projection machinery with existing descriptor tooling where appropriate, while
keeping provider restrictions separate from document semantics. Settle the supported
subset, optional values and scalar-root wrapping when implementing this slice. Generated
values should not carry row identities; insertion assigns `$id` through the normal
edit path. The output check must enforce that distinction rather than assume a provider
will omit IDs.

A bounded retry with the core's validation feedback is worth keeping. The original idea
was one retry, then a clear refusal if the answer still does not fit. Requests also need
cancellation, sensible input/output limits, and understandable unavailable/provider
errors. Those details belong with the service implementation rather than the camera/mic
milestone.

### Providers

FoundationModels is the native text/structured-generation candidate. Native speech
frameworks are the transcription candidate. Providers can be unavailable because of the
OS, device, language, configuration or model availability; apps need useful fallbacks.

Firebase AI Logic remains a cloud-provider candidate. Preserve the idea of shared
host-managed generation, but decide cloud opt-in, routing, spending controls and failure
behavior separately. On-device-first with cloud fallback is one possible policy, not a
requirement to silently send prompts off the device.

Provider adapters translate the supported schema projection and return results for Rust
to check. Model names, SDK requirements, App Check support in the signed Developer ID
build, and service setup should be verified when that work begins instead of frozen in
this design note.

## How it fits the current architecture

Rust owns capability declarations, acceptance, wire types and shared limits. TypeScript
comes from ts-rs, and UniFFI carries native types to Swift. `bun run schema:generate`
remains the generation workflow; the old TypeBox manifest and Swift JSON-decoding
approach is gone.

Declarations flow through the Rust build input, stored app definition and accepted app
model. The SDK's declaration projection must carry capabilities separately from catalog
metadata. Generated types carry the declaration to authors and the native host.

Camera/microphone requests use WebKit's permission delegate. Speech and AI would use
Rust-decoded page requests and typed host actions, with Swift supplying providers. The
SDK can expose `ctx.ai` / `ctx.speech` and matching `hitslop/svelte` helpers. Responses
return through the same typed boundary; Swift does not become another app/schema parser.

Keep provider calls tied to the requesting page's lifetime so a replaced or closed page
does not receive a stale reply. External effects stay outside the synchronous, retryable
document command evaluator. A page can call a service and then submit an ordinary edit.

`slop dev` already has a native Rust document owner, with the browser rendering the UI.
That does not automatically provide Apple services: each service needs an explicit
preview route or a clear unavailable result. The durable [browser host](browser-host.md)
is separate work. Reuse author-facing APIs where possible, with explicit availability
and frame permissions; a native capability need not exist in every host.

## Start with camera and microphone

First prove capture at `slop://app` in a signed Mac build. Then add declarations, native
admission, the required entitlements/usage strings, and declaration-based browser policy
in `slop dev`. There is no per-slop consent UI or grant store in this slice.

Build a small, initially unbundled Capture Notebook example:

- Camera preview and **Take Photo**.
- **Record Audio** and **Stop and Keep**.
- Taking a photo keeps it immediately; stopping a recording imports the completed clip.
- Saved photos and audio are available after reopening.
- Clear recording state, errors and device cleanup.

Use the existing attachment storage and limits. An unfinished recording is transient;
make that understandable rather than adding a new recording-recovery system to this
first example. Export views render saved content without requesting devices.

Checks should cover declarations, capture admission, native and development-preview
behavior, attachment round trips, and prompt-free exports. Use deterministic fixtures
for automation and signed-app manual checks for actual hardware and OS prompts.

As speech and AI land, add tests for typed output inference, descriptor projection and
validation, invalid answers and retry, unavailable providers, and cancellation on page
replacement/close. Follow the existing `bun run verify` workflow and run
`bun run verify --native` when native/FFI work lands. Update authoring guidance alongside
each implemented capability; examples here are not documentation of shipped APIs.

## Keep exploring

- **More intelligence:** OCR, translation, image generation, streamed responses and
  live dictation.
- **Device features:** notifications, MIDI, screen capture, location, fullscreen,
  screen wake and the native share sheet. Scheduled notifications could remain useful
  after a document window closes.
- **Spotify and other services:** host-managed accounts and credentials, with narrow
  operations exposed to slops. Documents carry useful references, never credentials.
  Check provider access constraints before committing to an integration.
- **Permissions:** grants tied to immutable app code, revocation and clear capability
  descriptions before [hosted distribution](share-links.md). The future store lives
  outside documents; duplicated or shared files do not transport grants.
- **Browser hosting:** explicit camera/microphone policy delegation to isolated app
  frames, plus availability behavior for services that only exist natively.

These ideas can become separate implementation slices as real slops need them. The
capabilities declaration should grow with working features, without committing now to
a generic plugin system or an arbitrary native-method bridge.
