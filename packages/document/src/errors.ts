import { OperationErrorBrand, PageErrorCodes } from "@hitslop/schema/constants";
import type { CoreErrorCode } from "@hitslop/schema/core";
import type { PageErrorCode } from "@hitslop/schema/page";
const brand = Symbol.for(OperationErrorBrand);
/** A document operation outcome. Use the guards across independently bundled apps. */
export class DocumentError extends Error {
  constructor(
    readonly code: PageErrorCode,
    message: string,
    readonly reason?: CoreErrorCode,
    readonly opIndex?: number,
  ) {
    super(message);
    this.name = "DocumentError";
    Object.defineProperty(this, brand, { value: true });
  }
}
export function isDocumentError(error: unknown): error is DocumentError {
  return (
    typeof error === "object" &&
    error !== null &&
    (error as any)[brand] === true &&
    (PageErrorCodes as readonly unknown[]).includes((error as any).code)
  );
}
/** Definite semantic refusal: no mutation was accepted. */
export function isRejected(error: unknown): error is DocumentError & { code: "rejected" } {
  return isDocumentError(error) && error.code === "rejected";
}
