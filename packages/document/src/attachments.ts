import { base64, hostCall } from "./bridge";
import { OperationRejectedError } from "./errors";

import { AttachmentIdPattern, AttachmentLimits as attachmentLimits } from "@hitslop/schema/constants";
export { attachmentLimits };
import type { AttachmentInfo, AttachmentRef } from "./abi";
export type { AttachmentInfo, AttachmentRef } from "./abi";
interface AttachmentStore {
  put(bytes: Uint8Array): Promise<AttachmentInfo>;
  read(id: string): Promise<Uint8Array>;
  list(): Promise<AttachmentInfo[]>;
}
const assertAttachmentID = (id: string) => {
  if (!new RegExp(AttachmentIdPattern).test(id)) throw new Error("Invalid attachment ID");
};
const checkSize = (size: number) => {
  if (size > attachmentLimits.file) throw new Error(`Attachment exceeds ${attachmentLimits.file >> 20} MiB`);
};
export class MemoryAttachments implements AttachmentStore {
  private files = new Map<string, Uint8Array>();
  private total = 0;
  async put(bytes: Uint8Array) {
    checkSize(bytes.length);
    const digest = await crypto.subtle.digest("SHA-256", new Uint8Array(bytes));
    const id = Array.from(new Uint8Array(digest), (n) => n.toString(16).padStart(2, "0")).join("");
    if (!this.files.has(id)) {
      if (
        this.files.size >= attachmentLimits.count ||
        this.total + bytes.length > attachmentLimits.total
      )
        throw new OperationRejectedError(
          "Document attachment limit reached (100 MiB or 256 files)",
        );
      this.files.set(id, bytes.slice());
      this.total += bytes.length;
    }
    return { id, byteLength: bytes.length };
  }
  async read(id: string) {
    assertAttachmentID(id);
    const bytes = this.files.get(id);
    if (!bytes) throw new Error("Attachment not found");
    return bytes.slice();
  }
  async list() {
    return [...this.files].map(([id, bytes]) => ({ id, byteLength: bytes.length }));
  }
}
export class HostAttachments implements AttachmentStore {
  async put(bytes: Uint8Array): Promise<AttachmentInfo> {
    checkSize(bytes.length);
    return hostCall({ method: "attachments.put", bytes: base64.encode(bytes) });
  }
  async read(id: string) {
    assertAttachmentID(id);
    return base64.decode((await hostCall({ method: "attachments.read", attachmentID: id })).bytes);
  }
  async list(): Promise<AttachmentInfo[]> {
    return (await hostCall({ method: "attachments.list" })).files;
  }
}
