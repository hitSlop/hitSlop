import type { OwnerDocument } from "./document";
import type { ThemeController } from "../theme-runtime";
import type { ownerAttachments } from "./attachments";

/** Renderer lifecycle; document and theme commands execute natively. */
export class OwnerSession {
  constructor(
    private doc: OwnerDocument<any>,
    private theme: ThemeController,
    private attachments: ReturnType<typeof ownerAttachments>,
  ) {}
  flush = () => this.doc.flush();
  prepareClose = () => this.doc.prepareClose();
  cancelClose = () => this.doc.cancelClose();
  /** Theme writes are validated and saved by the native owner; the page only applies them. */
  applyTheme(overrides: Record<string, string>) {
    this.theme.load(overrides);
  }
}
