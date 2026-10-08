import type { AttachmentInfo, AttachmentRef, Scope } from "../abi";
import type { ObjectNode } from "../schema";
import { current } from "./context";
export type { AttachmentInfo, AttachmentRef };

/** Host-owned immutable file storage, referenced from ordinary document fields. */
export const attachments = {
  /** Stores `file`, then runs `reference` as a synchronous collector to write the
   * reference: `attachments.import<typeof schema.descriptor>(file, (tx, ref) => …)`.
   * Unlike `void`, `void | undefined` refuses an async collector's promise. */
  import: <N extends ObjectNode = ObjectNode>(file: File, reference: (tx: Scope<N>, ref: AttachmentRef) => void | undefined) =>
    current().attachments.import(file, reference as never),
  /** A same-origin immutable URL, with the host-verified media type. */
  url: (id: string) => current().attachments.url(id),
  read: (id: string) => current().attachments.read(id),
};
