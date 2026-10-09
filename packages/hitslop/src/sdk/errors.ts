import { OperationErrorBrand } from "../schema/constants";
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
    brand in error && error[brand] === true &&
    "code" in error && typeof error.code === "string"
  );
}
/** Definite semantic refusal: no mutation was accepted. */
export function isRejected(error: unknown): error is DocumentError & { code: "rejected" } {
  return isDocumentError(error) && error.code === "rejected";
}
/** A command or page action refused with a message for the person. */
export function isRefused(error: unknown): error is DocumentError & { code: "rejected"; reason: "refused" } {
  return isRejected(error) && error.reason === "refused";
}
/** Stops a command, or a page action, with `message` for the person: "Enter a task." The
 * window shows it, nothing changes, and `slop call` reports it. Anything else a command
 * throws is a fault the host reports. */
export function refuse(message: string): never {
  throw new DocumentError("rejected", message, "refused");
}
