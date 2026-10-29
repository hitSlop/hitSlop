import { call } from "./bridge";
import { AttachmentIdPattern, SpeechLimits } from "../schema/constants";
import type { TinyContext } from "../sdk/abi";
import { isDocumentError, SpeechError } from "../sdk/errors";

/** Transcription by the host's speech service. The page names the stored recording by
 * its attachment ID; Rust checks the declaration and the host reads the bytes itself. */
export function ownerSpeech(): TinyContext["speech"] {
  return {
    async transcribe(clip, options = {}) {
      const attachment = typeof clip === "string" ? clip : clip?.id;
      if (typeof attachment !== "string" || !new RegExp(AttachmentIdPattern).test(attachment))
        throw new TypeError(
          "speech.transcribe needs a recording stored with attachments.import, or its ID",
        );
      const { locale } = options;
      if (
        locale !== undefined &&
        (typeof locale !== "string" ||
          locale.length > SpeechLimits.localeBytes ||
          !new RegExp(SpeechLimits.localePattern).test(locale))
      )
        throw new SpeechError(
          "unavailable",
          `"${String(locale)}" is not a locale identifier; use one such as "en-US"`,
          "locale",
        );
      let reply;
      try {
        reply = await call({
          method: "speech.transcribe",
          attachment,
          ...(locale === undefined ? {} : { locale }),
        });
      } catch (error) {
        if (!isDocumentError(error)) throw error;
        // The document closing or its page being replaced ended the request.
        const ended = error.code === "closing" || error.code === "owner_replaced";
        throw new SpeechError(ended ? "cancelled" : "failed", error.message);
      }
      const { outcome } = reply;
      if (outcome.status === "transcribed") return { text: outcome.text, locale: outcome.locale };
      throw outcome.status === "unavailable"
        ? new SpeechError("unavailable", outcome.message, outcome.reason)
        : new SpeechError(outcome.status, outcome.message);
    },
  };
}
