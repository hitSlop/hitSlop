/** Expected rejection before an edit is accepted. Uncaught rejections get host UI. */
export class OperationRejectedError extends Error {
  readonly code = "hitslop_operation_rejected";
  constructor(message: string, readonly reason?: import("@hitslop/schema/owner").CoreErrorCode, readonly opIndex?: number) {
    super(message);
    this.name = "OperationRejectedError";
  }
}
/** A page request outcome. `rejected`, `owner_replaced` and `closing` were not applied. */
export class OwnerError extends Error {
  constructor(readonly code: import("@hitslop/schema/owner").PageErrorCode, message: string) {
    super(message);
    this.name = "OwnerError";
  }
}

/** Definite semantic refusal: no mutation was accepted. */
export function isOperationRejection(error: unknown): boolean {
  return error instanceof OperationRejectedError;
}
