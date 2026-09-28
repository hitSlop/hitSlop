import type { OwnerDocument } from "./document";
import type { ThemeController } from "../theme-runtime";
import type { Request, Reply } from "../session-types";
import type { ownerAttachments } from "./attachments";

/** Renderer lifecycle and ancillary UI commands; document commands execute natively. */
export class OwnerSession {
  constructor(
    private doc: OwnerDocument<any>,
    readonly epoch: string,
    private theme: ThemeController,
    private attachments: ReturnType<typeof ownerAttachments>,
  ) {}
  flush = () => this.doc.flush();
  prepareClose = () => this.doc.prepareClose();
  cancelClose = () => this.doc.cancelClose();
  close = () => this.doc.close();
  async discardPending(): Promise<void> {
    throw new Error("Use native discard/reload recovery");
  }
  async handle(request: Request): Promise<Reply> {
    try {
      await this.flush();
      if (request.method === "hello") return { ok: true, epoch: this.epoch };
      if (request.method === "theme.get")
        return { ok: true, epoch: this.epoch, state: this.theme.get() };
      if (request.method === "theme.set" || request.method === "theme.reset") {
        if (request.epoch !== this.epoch)
          return { ok: false, code: "session_changed", error: "Session changed" };
        const state =
          request.method === "theme.set"
            ? await this.theme.set(request.values)
            : await this.theme.reset(request.token);
        return { ok: true, epoch: this.epoch, state };
      }
      return { ok: false, code: "rejected", error: "Document commands require the native owner" };
    } catch (error) {
      return { ok: false, code: "failed", error: String(error) };
    }
  }
}
