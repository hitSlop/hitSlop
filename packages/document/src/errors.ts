/** Expected rejection before an edit is accepted. Uncaught rejections get host UI. */
export class OperationRejectedError extends Error {
  readonly code = "hitslop_operation_rejected";
  constructor(message: string) {
    super(message);
    this.name = "OperationRejectedError";
  }
}
/** Live edits were accepted, but no required durable representation fits. */
export class DocumentFullError extends Error {
  constructor(limit: number) {
    const size =
      limit >= 1024 * 1024
        ? `${Math.round(limit / 1024 / 1024)} MiB`
        : `${Math.round(limit / 1024)} KiB`;
    // Hosts match the "Document is full" prefix to offer discarding unsaved edits.
    super(`Document is full (${size} limit); this edit cannot be saved`);
    this.name = "DocumentFullError";
  }
}

/** A page request outcome. `rejected`, `owner_replaced` and `closing` were not applied. */
export class OwnerError extends Error {
  constructor(
    readonly code: import("@hitslop/schema/owner").PageErrorCode,
    message: string,
  ) {
    super(message);
    this.name = "OwnerError";
  }
}
