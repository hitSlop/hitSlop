# Capabilities and permissions

The [public capability guide](../../apps/landing/src/content/docs/docs/guides/capabilities.mdx)
contains author examples, platform support, failure handling and future directions.
This page records the platform boundary; [runtime](../reference/runtime.md#security-boundaries)
describes the implementation in detail.

## Current contract

`defineSlop({ capabilities: ["camera", "microphone", "speech"], ... })` declares the
devices and host services an app uses. Omission means none. Rust owns the capability
type and stored acceptance, normalizes known values, and exposes them to the hosts.
Authoring rejects unknown values; opening a saved app ignores unknown capabilities
without granting them. Adding a declaration requires a rebuilt template and a new
document; it cannot be enabled through a data edit.

| Capability | Boundary | Availability |
| --- | --- | --- |
| `camera`, `microphone` | Web APIs, admitted by the native WebKit delegate or browser permission policy. | Interactive documents with the declared devices and OS/browser permission. |
| `speech` | Typed `speech.transcribe` host request carrying an attachment ID and optional locale, never audio bytes. | Owned native document page with on-device recognition support and speech permission. |

Camera/microphone requests are refused for embedded frames, snapshot renderers, and
pages being captured or replaced. Native capture also requires a visible window and
the main `slop://app` frame. A combined request needs both declarations. The development
preview and local browser copy delegate only declared devices to their app frame.

macOS consent is for hitSlop, not per document. There is no code-hash permission store
or per-slop consent UI yet. A declaration is an admission restriction, not consent.
Playback, file selection, attachments and allowed HTTPS requests do not acquire a new
capability requirement. The existing CSP, picker and storage boundaries still apply.

## Speech and stored media

Authors import recordings through `attachments.import` and keep their references in
ordinary document fields. Rust verifies the attachment and accepts m4a, mp3, wav, flac,
mp4 and mov for transcription. The host uses `SFSpeechRecognizer` with
`requiresOnDeviceRecognition`; unsupported on-device languages never fall back to an
upload. Support is checked before asking for permission. `SpeechAnalyzer` is not used.

The returned transcript is not saved automatically. Authors write it with normal
document handles and guard against a recording changing while recognition runs.
Attachment limits remain 10 MiB each, 100 MiB and 256 files per document; transcripts
are bounded to 1 MiB. Ogg/WebM storage does not imply native transcription support.

`isSpeechError` recognizes cross-bundle errors with codes `unavailable`, `cancelled` and
`failed`. Unavailability reasons are `notDeclared`, `host`, `system`, `locale` and
`permission`. Development previews, browser copies and snapshot renderers return
`unavailable`/`host`. Native work belongs to its page: close, replacement, reload and
capture cancel pending recognition. A recording remains usable without a transcript.

## Validation and future work

Keep regressions at their owning boundaries: Rust declaration/request/media acceptance,
SDK/shell outcome delivery, Swift capture admission and speech lifecycle. Native tests
use a controlled recognizer rather than requiring a person's speech permission. Check
real device prompts, denied permission and on-device language support by hand in the
app; browser preview alone cannot qualify speech. Do not add per-template test suites.

[Code-bound permissions](../ideas.md#permissions-bound-to-the-apps-code) and additional
devices such as MIDI are proposals. Worker/worklet asset support remains in
[Direction](../roadmap.md#next). Browser speech and a newer native recognition backend
are not implemented; no future capability names are accepted in authoring today.
