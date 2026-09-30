import type { AttachmentInfo, AttachmentRef, Scope } from "../abi";
import type { ObjectNode } from "../schema";
import { current } from "./context";
export type { AttachmentInfo, AttachmentRef };

/** Host-owned immutable file storage, referenced from ordinary document fields. */
export const attachments = {
  /** Stores `file`, then runs `reference` as a synchronous collector to write the
   * reference: `attachments.import<typeof schema.descriptor>(file, (tx, ref) => …)`. */
  import: <N extends ObjectNode = ObjectNode>(file: File, reference: (tx: Scope<N>, ref: AttachmentRef) => void) =>
    current().attachments.import(file, reference as never),
  /** Pass the saved `mimeType` so object URLs for images and media resolve with the right type. */
  read: (id: string, options?: { type?: string }) => current().attachments.read(id, options),
  list: () => current().attachments.list(),
};
