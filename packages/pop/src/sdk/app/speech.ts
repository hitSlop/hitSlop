import type { SpeechClip, SpeechOptions, SpeechTranscript } from "../abi";
import { current } from "./context";
export type { SpeechClip, SpeechOptions, SpeechTranscript };
// A type only, like DocumentError: the shell's errors come from another bundle, so
// authors test them with `isSpeechError`, never `instanceof`.
export { isSpeechError, type SpeechError, type SpeechUnavailableReason } from "../errors";

/** On-device speech recognition for recordings the document stores. Declare `"speech"` in
 * `capabilities` in pop.ts. */
export const speech = {
  /** Transcribes a recording stored with `attachments.import`: pass its reference or its
   * ID. Rejects with a `SpeechError`: `unavailable` (see its `reason`), `cancelled` or
   * `failed`. Saving the text is an ordinary edit:
   * `const { text } = await speech.transcribe(ref, { locale: "en-US" })`. */
  transcribe(clip: SpeechClip, options?: SpeechOptions): Promise<SpeechTranscript> {
    return current().speech.transcribe(clip, options);
  },
};
