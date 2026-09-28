import type { AttachmentInfo, AttachmentRef } from "../attachments";
import { current } from "./context";
export type { AttachmentInfo, AttachmentRef };

/** Host-owned immutable file storage, referenced from ordinary document fields. */
export const attachments = {
  import: (file: File, options: { commit(ref: AttachmentRef): void }) =>
    current().attachments.import(file, options),
  /** Pass the saved `mimeType` so object URLs for images and media resolve with the right type. */
  read: (id: string, options?: { type?: string }) => current().attachments.read(id, options),
  list: () => current().attachments.list(),
};
