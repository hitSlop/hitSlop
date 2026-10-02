import { OperationErrorBrand } from "@hitslop/schema/constants";
import type { CoreErrorCode } from "@hitslop/schema/core";
import type { PageErrorCode } from "@hitslop/schema/page";
const brand = Symbol.for(OperationErrorBrand);
/** A document operation outcome. Use the guards across independently bundled apps.
 * Codes and reasons may grow: an app built before a code existed still recognizes the
 * outcome, and sees the new code as an unfamiliar string. */
export class DocumentError extends Error {
  readonly code: PageErrorCode | (string & {});
  readonly reason?: CoreErrorCode | (string & {});
  readonly opIndex?: number;
  constructor(code: PageErrorCode, message: string, reason?: CoreErrorCode, opIndex?: number) {
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
    (error as any)[brand] === true &&
    typeof (error as any).code === "string"
  );
}
/** Definite semantic refusal: no mutation was accepted. */
export function isRejected(error: unknown): error is DocumentError & { code: "rejected" } {
  return isDocumentError(error) && error.code === "rejected";
}
