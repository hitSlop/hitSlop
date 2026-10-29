import { OperationErrorBrand, SpeechErrorBrand } from "../schema/constants";
import type { SpeechUnavailable } from "../wire/page.generated";
import type { CoreErrorCode } from "../schema/core";
import type { OutcomeCode } from "../schema/values";
const brand = Symbol.for(OperationErrorBrand);
/** A document operation outcome. Use the guards across independently bundled apps.
 * Codes and reasons may grow: an app built before a code existed still recognizes the
 * outcome, and sees the new code as an unfamiliar string. */
export class DocumentError extends Error {
  readonly code: OutcomeCode | (string & {});
  readonly reason?: CoreErrorCode | (string & {});
  readonly opIndex?: number;
  constructor(code: OutcomeCode, message: string, reason?: CoreErrorCode, opIndex?: number) {
    super(message);
    this.name = "DocumentError";
    this.code = code;
    this.reason = reason;
    this.opIndex = opIndex;
    Object.defineProperty(this, brand, { value: true });
  }
}
export function isDocumentError(error: unknown): error is DocumentError {
  return (
    typeof error === "object" &&
    error !== null &&
    brand in error &&
    error[brand] === true &&
    "code" in error &&
    typeof error.code === "string"
  );
}
/** Definite semantic refusal: no mutation was accepted. */
export function isRejected(error: unknown): error is DocumentError & { code: "rejected" } {
  return isDocumentError(error) && error.code === "rejected";
}
/** A command or page action refused with a message for the person. */
export function isRefused(
  error: unknown,
): error is DocumentError & { code: "rejected"; reason: "refused" } {
  return isRejected(error) && error.reason === "refused";
}
/** Stops a command, or a page action, with `message` for the person: "Enter a task." The
 * window shows it, nothing changes, and `tiny call` reports it. Anything else a command
 * throws is a fault the host reports. */
export function refuse(message: string): never {
  throw new DocumentError("rejected", message, "refused");
}

const speechBrand = Symbol.for(SpeechErrorBrand);
/** Why speech is unavailable: the app did not declare `speech` (`notDeclared`); this host
 * has no speech service, as in `tiny dev`, a browser copy or an export (`host`); the OS
 * cannot transcribe now (`system`); the provider cannot decode the recording's format
 * (`format`); the locale has no on-device recognizer, and tinys
 * never sends a recording off the device instead (`locale`); or the person declined
 * speech recognition for tinys (`permission`). Reasons may grow. */
export type SpeechUnavailableReason = SpeechUnavailable;
/** A transcription that produced no transcript. `unavailable` will not succeed by retrying
 * as is (see `reason`); `cancelled` ended because the page was replaced or closed, or an
 * export began; `failed` could not read or recognize the recording. Codes may grow. */
export class SpeechError extends Error {
  readonly code: "unavailable" | "cancelled" | "failed" | (string & {});
  readonly reason?: SpeechUnavailableReason | (string & {});
  constructor(
    code: "unavailable" | "cancelled" | "failed",
    message: string,
    reason?: SpeechUnavailableReason,
  ) {
    super(message);
    this.name = "SpeechError";
    this.code = code;
    if (reason !== undefined) this.reason = reason;
    Object.defineProperty(this, speechBrand, { value: true });
  }
}
/** Recognizes a `SpeechError` thrown by the separately bundled page shell. */
export function isSpeechError(error: unknown): error is SpeechError {
  return (
    typeof error === "object" &&
    error !== null &&
    speechBrand in error &&
    error[speechBrand] === true &&
    "code" in error &&
    typeof error.code === "string"
  );
}
