import type { SpeechClip, SpeechOptions, SpeechTranscript } from "../abi";
import { SpeechError } from "../errors";
import { current } from "./context";
export type { SpeechClip, SpeechOptions, SpeechTranscript };
// A type only, like DocumentError: the shell's errors come from another bundle, so
// authors test them with `isSpeechError`, never `instanceof`.
export { isSpeechError, type SpeechError, type SpeechUnavailableReason } from "../errors";

/** On-device speech recognition for recordings the document stores. Declare `"speech"` in
 * `capabilities` in slop.ts. */
export const speech = {
  /** Transcribes a recording stored with `attachments.import`: pass its reference or its
   * ID. Rejects with a `SpeechError`: `unavailable` (see its `reason`), `cancelled` or
   * `failed`. Saving the text is an ordinary edit:
   * `const { text } = await speech.transcribe(ref, { locale: "en-US" })`. */
  transcribe(clip: SpeechClip, options?: SpeechOptions): Promise<SpeechTranscript> {
    const service = (current() as Partial<ReturnType<typeof current>>).speech;
    // A hitSlop older than this SDK has no speech service.
    if (!service) return Promise.reject(new SpeechError("unavailable", "This hitSlop cannot transcribe speech; update hitSlop", "host"));
    return service.transcribe(clip, options);
  },
};
