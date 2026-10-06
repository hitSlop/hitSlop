# Slop capabilities: microphone, camera, speech-to-text and AI

Status: planned (2026-10-05). Not started. Nothing is committed without asking.

## Why

Slops are third-party web apps that run at `slop://app` in a WKWebView owned by
`DocumentSession`. Today they cannot reach any device or host intelligence:

- `apps/apple/App/macOS/hitSlop.entitlements` is an empty `<dict/>`, and the app runs
  with the hardened runtime. Without `device.audio-input` or `device.camera`, macOS's
  privacy system (TCC) never prompts and capture fails.
- `apps/apple/App/macOS/Info.plist` has no usage strings.
- `DocumentSession` does not implement `requestMediaCapturePermissionFor`. WebKit's
  default is to deny.
- The manifest is `Strict` (`additionalProperties: false`), so an author cannot declare
  anything.
- WKWebView has no Web Speech recognition, no Web Notifications and no on-device model.

Goal: slops can use the mic and webcam through standard web APIs. They also get
speech-to-text and AI text generation, including structured output, as host services.
AI runs on-device first and falls back to Firebase AI Logic. One `capabilities`
declaration grows to cover later capabilities (image generation, notifications and so
on).

## Decisions

- **Declare to use.** A slop lists capabilities in `slop.ts`. A declared capability is
  granted.
- **No consent UI and no grant store in this pass.** macOS still shows its own one-time
  privacy prompt per app. A consent store keyed by code hash
  ([ideas](../docs/ideas.md#permissions-bound-to-the-apps-code)) must land before
  sharing or a hosted catalog.
- **Mic and camera use the web platform.** That means `getUserMedia` and
  `MediaRecorder`, so web audio libraries such as Tone, Strudel and Webamp work
  unchanged.
- **Speech and AI are host services on `ctx`,** over the existing page bridge.
- **Structured output uses the `s.*` vocabulary authors already use,** not raw JSON
  Schema. The Rust core owns the mapping and the check.
- **Pre-launch, so the markers stay where they are:** `packageFormat` 1 and
  `runtimeABI` 1, with no migrations.
- **The proof is a Swift fixture and probe.** Example slops stay untouched: only
  `quick-checklist` and `shape-lab` are active, and the rest wait in
  `examples/archive`.

## Already verified

- `slop://app` is a secure context. The shell's `crypto.subtle.digest` already works
  there (`packages/shell/src/attachments.ts:23`), so `navigator.mediaDevices` should
  exist. Phase 6 asserts this.
- The installed Xcode 27 SDK includes FoundationModels. The deployment target is macOS
  15.2 (`apps/apple/project.yml`), so model code needs `@available(macOS 26, *)`.
- `firebase-ios-sdk` is pinned to 12.18.0 and ships the `FirebaseAILogic` and
  `FirebaseAppCheck` products.
- Firebase enforces App Check for AI Logic from 2026-11-02. Gemini 2.5 models shut down
  in October 2026, so use `gemini-3.8-flash`.

## Author experience

```ts
// slop.ts
export default defineSlop({
  title: "Flashcards",
  // …
  capabilities: ["microphone", "speech", "ai"],
});
```

```svelte
<script lang="ts">
  import { s, isDocumentError } from "@hitslop/document";
  import { ai, speech } from "@hitslop/document/svelte";
  import doc from "./schema";

  let topic = $state("");
  let notice = $state("");

  // Structured output: typed from the node, ready to insert.
  async function generate() {
    try {
      const { cards } = await ai.generate({
        prompt: `10 flashcards about ${topic}`,
        output: s.object({ cards: s.list(s.object({ front: s.string(), back: s.string() })) }),
      });
      await doc.change((tx) => { for (const c of cards) tx.fields.cards.insert(c); });
    } catch (e) {
      notice = isDocumentError(e) && e.code === "unavailable" ? "AI isn't available on this Mac" : String(e);
    }
  }

  // Microphone through the web platform, transcription through the host.
  let recorder: MediaRecorder | undefined;
  async function record() {
    const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    const chunks: Blob[] = [];
    recorder = new MediaRecorder(stream);
    recorder.ondataavailable = (e) => chunks.push(e.data);
    recorder.onstop = async () => {
      stream.getTracks().forEach((t) => t.stop());
      topic = await speech.transcribe(new Blob(chunks, { type: recorder!.mimeType }));
    };
    recorder.start();
  }
</script>
```

Other forms:

- `ai.generate({ prompt })` returns plain text.
- `output: s.enum(["positive", "negative"])` classifies and returns the chosen string.
- `instructions` sets the system prompt.

## Phase 1: the manifest's `capabilities`

**Files**

- `packages/schema/src/constants.ts`: add
  `export const SlopCapabilities = ["microphone", "camera", "speech", "ai"] as const;`
  next to `SlopCategories`.
- `packages/schema/src/manifest.ts`: add this to `manifestFields`:

  ```ts
  capabilities: Type.Optional(
    Type.Array(Type.Enum(SlopCapabilities, { title: "SlopCapability" }), {
      uniqueItems: true,
      maxItems: SlopCapabilities.length,
    }),
  ),
  ```

- Run `bun run schema:generate`. It regenerates
  `packages/schema/generated/manifest.schema.json`,
  `HitSlopCore/Generated/SlopManifest.generated.swift` (`capabilities:
  [SlopCapability]?`) and the contracts. Never edit generated files.

**These follow with no edits**

- The `Slop` type behind `defineSlop` is `Omit<SlopManifest, …>`
  (`packages/document/src/slop.ts`).
- `normalizeApp` spreads the remaining `slop.ts` fields into `parseManifest`
  (`packages/cli/src/build.ts:84-92`).
- The Rust open-time check compiles the generated schema
  (`crates/hitslop-core/src/manifest.rs`, the `jsonschema::validator` macro).
- Swift `SlopFile.manifest` (`HitSlopCore/SlopFile.swift:44`) is the generated type.

**Tests**

- `packages/schema/tests/manifest.test.ts` accepts each known value and rejects an
  unknown value, a duplicate and a non-array.
- `crates/hitslop-core/tests/manifest.rs` checks that the native check agrees, both on
  a manifest with `capabilities` and on one without.

## Phase 2: microphone and camera

Everything here is in
`apps/apple/Packages/HitSlopApple/Sources/HitSlopDocument/DocumentSession.swift`
unless noted.

1. **One admission rule.** The same guard appears three times today: in the open panel
   (~l.718), in downloads (~l.731) and in `decidePolicyFor` (~l.755). It checks:
   - `webView === liveWebView`
   - `allowsFileSelection`, `!capturing`, `isReady`, `!closing`, `!closed` and
     `!rendererDead`
   - the frame is the main frame, with origin `slop://app`

   Extract it once, use it in those three places and in every new handler, and keep it
   the single owner of the rule:

   ```swift
   private func admitsPageRequest(from frame: WKFrameInfo) -> Bool
   ```

   `allowsFileSelection` already means "interactive": it is `purpose == .interactive`
   (~l.263), so background renders and the CLI helper's screenshot and export are
   refused. Consider renaming it to `interactive`.
2. **Declaration check.**

   ```swift
   func declares(_ c: SlopCapability) -> Bool {
     file.manifest.capabilities?.contains(c) == true
   }
   ```

3. **A new `WKUIDelegate` method**,
   `webView(_:requestMediaCapturePermissionFor:initiatedByFrame:type:decisionHandler:)`:

   ```swift
   nonisolated static func mediaDecision(
     _ type: WKMediaCaptureType, declared: Set<SlopCapability>, admitted: Bool
   ) -> WKPermissionDecision {
     guard admitted else { return .deny }
     let needed: Set<SlopCapability> = switch type {
       case .microphone: [.microphone]
       case .camera: [.camera]
       case .cameraAndMicrophone: [.camera, .microphone]
       @unknown default: [.camera, .microphone, .speech, .ai] // unknown: never granted
     }
     return needed.isSubset(of: declared) ? .grant : .deny
   }
   ```

   - It never returns `.prompt`, because WebKit's own prompt would name `slop://app`.
   - Embedded HTTPS frames such as YouTube are never admitted.
   - Tighten the `@unknown default` so it always denies.
4. **Entitlements** (`apps/apple/App/macOS/hitSlop.entitlements`):
   `com.apple.security.device.audio-input` and `com.apple.security.device.camera`.
   The app stays unsandboxed.
5. **Info.plist** (`apps/apple/App/macOS/Info.plist`):
   - `NSMicrophoneUsageDescription`: "A document you opened wants to use the
     microphone."
   - `NSCameraUsageDescription`: "A document you opened wants to use the camera."
   - `NSSpeechRecognitionUsageDescription`: "A document you opened wants to transcribe
     speech."
6. **No change to the page policy.** A MediaStream is not a URL, and playing back a
   recording uses `blob:`, which `media-src` already allows. Leave
   `SchemeHandler.swift` (CSP) and `packages/cli/src/dev.ts` alone. In `slop dev`, the
   browser shows its own prompt.
7. **Check xcodegen.** `project.yml` already points `CODE_SIGN_ENTITLEMENTS` and
   `INFOPLIST_FILE` at these files, so it should need no edit. Verify that
   `xcodegen generate` leaves the project as it is.

## Phase 3: host services on the page bridge

### Wire (`packages/schema/src/page.ts`)

Requests stay flat strings and numbers, because `PageRequest.init`
(`HitSlopDocument/PageRequest.swift`) rejects anything else.

```ts
"ai.generate": Strict({
  method: T.Literal("ai.generate"),
  prompt: T.String({ minLength: 1, maxLength: AILimits.prompt }),
  instructions: T.Optional(T.String({ maxLength: AILimits.instructions })),
  output: T.Optional(T.String({ minLength: 2, maxLength: AILimits.output })), // node JSON
}),
"speech.transcribe": Strict({
  method: T.Literal("speech.transcribe"),
  bytes: AttachmentBytesSchema,                      // base64 audio, as attachments.put
  locale: T.Optional(T.String({ maxLength: 35 })),   // BCP 47
}),
```

The results go in `PageResults`:

- `ai.generate`: `{ text: string }`. When `output` is given, `text` is the canonical JSON
  that the core checked.
- `speech.transcribe`: `{ text: string }`.

Other changes:

- `constants.ts`: add
  `AILimits = { prompt: 32 * 1024, instructions: 8 * 1024, output: 16 * 1024 }`.
- `PageErrorCodes`: add `"unavailable"`. Error codes grow additively, and apps treat
  unknown ones as outcomes. It means one of:
  - the OS can't run the model,
  - no provider is configured,
  - the browser preview, which has no host.

  These cases use the existing `rejected` code with a message:
  - the capability is not declared,
  - a request is already in flight,
  - the page is capturing or closing,
  - an answer still doesn't fit its shape after the retry.
- Run `bun run schema:generate`. The exhaustive Swift switch over `PageMethod` then
  requires the two new cases.

### Host (`DocumentSession.swift`, the switch at ~l.399)

Add a new file, `HitSlopDocument/SlopHostServices.swift`:

```swift
public protocol SlopHostServices: Sendable {
  /// `schema` is the core's provider-neutral JSON Schema (Phase 3a), or nil for text.
  func generate(prompt: String, instructions: String?, schema: String?) async throws -> String
  func transcribe(audio: Data, locale: String?) async throws -> String
}
public enum SlopHostServiceError: Error { case unavailable(String) }
```

- `DocumentSession` gains `public static var services: (any SlopHostServices)?`. The
  app sets it at launch, and tests inject fakes. The CLI helper (`hitslop-native`)
  never sets it, so it answers `unavailable`.
- Each case follows the `.config` pattern (~l.402): `let page = message.webView`, then a
  `Task { @MainActor }` that drops the reply if `page !== liveWebView`. In order:
  1. Admit the request with `admitsPageRequest(from: message.frameInfo)`.
  2. Require `declares(.ai)` or `declares(.speech)`.
  3. Allow one request in flight per session per service; a second gets `rejected`
     ("busy"). This is a cheap abuse limit while there is no consent.
  4. For `output`, ask the core for the schema (Phase 3a).
  5. Call the provider.
  6. Check the answer with the core, and retry once if it doesn't fit.
  7. Reply.
- Keep the in-flight `Task`s on the session. Cancel them in `close`/`finishClose` and in
  `replaceWebView`/`destroyWebView`, as the file picker is cancelled today.
- **Errors.** `SlopHostServiceError.unavailable` maps to the page code `unavailable`.
  Everything else maps to `rejected` with the provider's message, trimmed to
  `Limits.errorText`. Extend `DocumentOwner.pageFailure` / `OwnerError`
  (`OwnerError.swift`) with that one case; don't add a parallel mapper.

### Shell and SDK

- `packages/document/src/abi.ts` adds two members to `SlopContext`:

  ```ts
  readonly ai: {
    generate(request: { prompt: string; instructions?: string }): Promise<string>;
    generate<N extends Node>(request: { prompt: string; instructions?: string; output: N }): Promise<Input<N>>;
  };
  readonly speech: {
    transcribe(audio: Blob, options?: { locale?: string }): Promise<string>;
  };
  ```

- `packages/shell/src/boot.ts`, `createContextV1`, implements both through `call()`:
  - `output` is sent as `JSON.stringify(node)`, and structured replies are parsed with
    `JSON.parse(text)`.
  - Audio is base64-encoded the same way `attachments.put` does it
    (`packages/shell/src/attachments.ts:49`). Share that helper; don't copy it.
  - In the browser preview, both reject with a `DocumentError` whose code is
    `unavailable`.
- Author helpers go in `packages/document/src/app/ai.ts` and `speech.ts`. Each forwards
  to `current().ai` and `current().speech`, the same way `attachments.ts` does. Export
  them from wherever `attachments` is exported (`./svelte`).
- Tests:
  - Update `packages/shell/tests/platform-contracts.types.ts`: both overloads, and
    `Input<N>` inference for objects, lists and enums.
  - Shell tests: the preview rejects with `unavailable`, structured replies are parsed,
    and the audio is encoded.

## Phase 3a: structured output (`s.*`, checked by the core)

**Why not raw JSON Schema**

- One vocabulary has one owner. Authors already write `s.*`
  (`packages/document/src/schema.ts:107`).
- The core already checks node shapes (`Node::check`) and values (`Node::validate`)
  (`crates/hitslop-core/src/descriptor.rs:89,162`).
- The result type comes free from `Input<N>`.
- Gemini silently ignores unsupported keywords, and Apple's `DynamicGenerationSchema`
  is narrower still. Raw JSON Schema would mislead authors and need a second validator.

**Rust** (`crates/hitslop-core/src/descriptor.rs`). These are public, exported through
`crates/hitslop-core-ffi/src/lib.rs`, and have no WASM export, since the preview has no
host.

- `pub fn output_schema(node: &str) -> Result<String>`:
  1. Parse the node, run `check(0)` and enforce `AILimits.output`.
  2. Map it to the JSON Schema subset both providers accept:

     | node | schema |
     | --- | --- |
     | `object` | `{type: object, properties, required}` |
     | `list` | `{type: array, items}` |
     | `string` | `{type: string, maxLength?}` (a hint) |
     | `text` | `{type: string}` |
     | `boolean` | `{type: boolean}` |
     | `number` | `{type: number, minimum?, maximum?}` (hints) |
     | `integer`, `counter` | `{type: integer, minimum?, maximum?}` (hints) |
     | `enum` | `{type: string, enum}` |
     | `optional` | the inner node, left out of `required` |
     | `record` | refused (`InvalidSchema`): a model can't be held to dynamic keys |

  3. A root that is not an object is wrapped as `{value: …}`, because providers want an
     object root.
- `pub fn check_output(node: &str, json: &str) -> Result<String>`:
  1. Unwrap the root wrapper.
  2. Run `Node::validate`, which enforces the bounds the providers treated as hints.
  3. Return canonical JSON (`$id` stays absent; inserting assigns it).
- Rust tests cover:
  - the mapping table,
  - `record` being refused,
  - outputs that are out of range, missing a field, or carrying an unknown field,
  - the enum and scalar-root wrapper round trip,
  - the size and depth limits.

**Swift adapters.** Each provider translates the subset mechanically:

- **Firebase:** `GenerationConfig(responseMIMEType: "application/json", responseSchema:
  Schema…)`. Firebase AI Logic treats every field as required unless it is listed in
  `optionalProperties`, which matches `s.optional`.
- **FoundationModels:** `DynamicGenerationSchema` (object properties, arrays, an anyOf
  of strings for enums) plus `GenerationSchema(root:dependencies:)`. Read the answer
  from `GeneratedContent`'s JSON.

**Retry.** If `check_output` fails, the host retries once and appends the core's message
to the prompt. If the answer still doesn't fit, it returns `rejected` ("The answer
didn't fit the requested shape").

## Phase 4: on-device providers (new SwiftPM target `HitSlopIntelligence`)

- `apps/apple/Packages/HitSlopApple/Package.swift`: add the `HitSlopIntelligence`
  target, depending on `HitSlopDocument`, and link it into the app the way
  `HitSlopFirebase` is (`project.yml`).
- **Text** (`OnDeviceTextGenerator`), `@available(macOS 26, *)`:
  - Throw `.unavailable` unless `SystemLanguageModel.default.availability ==
    .available`.
  - Create a `LanguageModelSession(instructions:)`, then call `respond(to:)` or
    `respond(to:schema:)`.
  - Map a context-window overflow or a guardrail refusal to `rejected` with the
    message.
- **Speech** (`OnDeviceTranscriber`):
  1. Write the bytes to a temp file and delete it afterwards.
  2. Call `SFSpeechRecognizer.requestAuthorization` once. This is the OS prompt.
  3. Run `SFSpeechRecognizer(locale:)` with an `SFSpeechURLRecognitionRequest`. Set
     `requiresOnDeviceRecognition` when `supportsOnDeviceRecognition`.
  4. Throw `.unavailable` when no recognizer is available for the locale.

  Moving to SpeechAnalyzer/SpeechTranscriber on macOS 26 is a follow-up.
- **Routing** (`RoutedHostServices(onDevice:cloud:)`):
  - AI uses on-device when it is available, otherwise the cloud provider, otherwise
    `.unavailable`.
  - Speech is on-device only.
- **Launch.** Set `DocumentSession.services` where the app calls
  `HitSlopFirebase.configure()`.

## Phase 5: Firebase AI Logic as the cloud provider (can ship separately)

- `Package.swift`: add the `FirebaseAILogic` and `FirebaseAppCheck` products to the
  `HitSlopFirebase` target.
- Add `FirebaseTextGenerator: SlopHostServices`:

  ```swift
  FirebaseAI.firebaseAI(backend: .googleAI())
    .generativeModel(
      modelName: Self.model,
      generationConfig: …,
      systemInstruction: instructions.map { ModelContent(role: "system", parts: $0) })
    .generateContent(prompt)
  ```

  `Self.model` is `"gemini-3.8-flash"`. Keep the model name in one constant.
- `HitSlopFirebase.configure()` currently skips `FirebaseApp.configure()` in DEBUG
  (l.11). AI needs the app to be configured:
  - Configure it in every run except tests.
  - Keep Analytics and Crashlytics collection release-only.
  - Install App Check before configuring. It is mandatory from 2026-11-02. Use
    `AppCheckDebugProvider` in DEBUG, and App Attest (or DeviceCheck) in release.
    **Verify it works for the Developer ID build that Sparkle ships.**
- These steps are the user's, in the Firebase console. They are outward-facing, so they
  aren't done from here:
  - enable AI Logic (Gemini Developer API),
  - enforce App Check,
  - set per-user quotas and a budget alert,
  - register the App Check debug token.
- This provider is also the path to image generation later (`gemini-3.1-flash-image` or
  Imagen).

## Phase 6: proof

**`apps/apple/Packages/HitSlopApple/Tests/HitSlopDocumentTests/CapabilityTests.swift`**
follows `PagePolicyProbeTests.swift`: `Fixtures.stage()`, `Fixtures.updateManifest`,
and a window so WebKit treats the page as visible.

- **Page probe:**
  - `isSecureContext` is true.
  - `typeof navigator.mediaDevices?.getUserMedia === "function"`.
  - Record what `MediaRecorder.isTypeSupported('audio/mp4')` and `'audio/webm'` return,
    as evidence.
- **`mediaDecision` table:**
  - nothing declared: deny;
  - mic declared: mic granted, camera denied;
  - camera plus mic: both must be declared;
  - a frame that isn't admitted: deny.
- **The bridge, end to end** (`callAsyncJavaScript` → `ctx.ai` / `ctx.speech`), with a
  fake `SlopHostServices`:
  - Declared returns the fake text; undeclared returns `rejected`.
  - `services == nil` returns `unavailable`.
  - While `capturing`, it returns `rejected`.
  - A second concurrent call returns `rejected` ("busy").
  - A reply after `replaceWebView` is dropped, and the task is cancelled.
  - Structured output: a bad answer and then a good one resolves to the typed value;
    two bad answers end in `rejected`.

Phases 1, 3 and 3a list their own schema, Rust and shell tests. A regression test must
fail before its fix for the intended reason. Tests don't cover private call sequences.

## Phase 7: docs

- `docs/reference/runtime.md:95`: replace "Camera/microphone grants are not part of this
  release" with the declare-to-use rule, and say plainly that consent is not yet asked.
- Landing guides:
  - `apps/landing/src/content/docs/docs/guides/manifest-and-windows.mdx`: the
    `capabilities` field.
  - `files-and-web.mdx`: mic and camera, `ctx.speech`, `ctx.ai` (text and `output`),
    and fallbacks for `unavailable`.
  - Keep repository internals out.
- The authoring skill references under `packages/cli/skills/`, then run
  `bun run skills:build`.
- `docs/ideas.md`:
  - Update "Permissions bound to the app's code": declaration has landed, and the
    consent and grant store is next. It is required before sharing or a hosted catalog,
    because a declared capability is granted without asking and cloud AI spends money.
  - Add a "More capabilities" entry:
    - notifications (UNUserNotificationCenter, scheduled so they fire after the window
      closes),
    - element fullscreen and screen wake,
    - MIDI (CoreMIDI),
    - screen capture,
    - location,
    - the share sheet,
    - OCR and translation (Vision, Translation),
    - image generation,
    - streamed AI output,
    - live dictation,
    - an `into:` shortcut, where `ai.generate({ prompt, into: doc.fields.cards })`
      derives `output` from the handle's node and inserts the result.
- `docs/roadmap.md`: list this under "Open now".

## Order

Phase 1 comes first; every later phase needs it. Phases 2 and 3 are independent. Phase
3a depends on 3, Phase 4 on 3 and 3a, and Phase 5 on 4. Phase 6 is written alongside
each phase. Phase 7 comes last.

## Verification

```sh
bun run schema:generate && bun run schema:check
bun run check && bun run test
cargo test --locked --workspace
bun run build && bun run swift:test && bun run test:native
```

Manual, in a built app:

1. Make a copy of a fixture slop that declares `microphone` and `camera`.
   `getUserMedia` succeeds after the macOS prompt.
2. Remove the declaration. `getUserMedia` is now rejected.
3. Check that `slop export` (PNG and PDF) and the CLI screenshot never prompt.
4. Check that an embedded HTTPS frame cannot get the mic.
5. On macOS 26 with Apple Intelligence on, `ctx.ai.generate` returns on-device text and
   a structured `output`.
6. With on-device off and Firebase set up, the same calls return Gemini answers.
7. A recorded clip transcribes.

## Risks and open checks

- **TCC attribution.** When the app runs from Xcode, TCC attributes the prompt to Xcode.
  Test the privacy prompts with a signed build.
- **WebKit capture in a custom scheme.** Phase 6's probe confirms that `mediaDevices`
  exists at `slop://app`. If it doesn't, stop and reassess before Phase 2 lands.
- **`MediaRecorder` output format.** WKWebView most likely produces `audio/mp4` (AAC).
  `SFSpeechURLRecognitionRequest` reads it, but confirm with the probe.
- **App Attest under Developer ID.** If it is not supported, use DeviceCheck.
- **Cost and abuse.** Any slop that declares `ai` can spend quota without asking. The
  in-flight limit, App Check and per-user quotas contain it until consent lands.
- **Sending data off the Mac.** Cloud fallback sends prompts off the device. Document it
  in the runtime reference; the consent work decides whether to ask first.

## Out of scope

- Consent UI and the grant store
- Notifications, fullscreen and screen wake
- Image generation
- Streaming AI output and live dictation
- The `into:` shortcut
- Raw JSON Schema output
- Porting archived slops
- Raising the markers
