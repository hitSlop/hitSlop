import { base64, call } from "./bridge";
import { DocumentError } from "@hitslop/document/internal";

import { AttachmentIdPattern, AttachmentLimits } from "@hitslop/schema/constants";
import type { AttachmentInfo, SlopContext } from "@hitslop/document/abi";
import type { OwnerDocument } from "./owner/document";
interface AttachmentStore {
  put(bytes: Uint8Array): Promise<AttachmentInfo>;
  read(id: string): Promise<Uint8Array>;
}
const assertAttachmentID = (id: string) => {
  if (!new RegExp(AttachmentIdPattern).test(id)) throw new Error("Invalid attachment ID");
};
const checkSize = (size: number) => {
  if (size > AttachmentLimits.file)
    throw new Error(`Attachment exceeds ${AttachmentLimits.file >> 20} MiB`);
};
class MemoryAttachments implements AttachmentStore {
  private files = new Map<string, Uint8Array>();
  private total = 0;
  async put(bytes: Uint8Array) {
    checkSize(bytes.length);
    const digest = await crypto.subtle.digest("SHA-256", new Uint8Array(bytes));
    const id = Array.from(new Uint8Array(digest), (n) => n.toString(16).padStart(2, "0")).join("");
    if (!this.files.has(id)) {
      if (
        this.files.size >= AttachmentLimits.count ||
        this.total + bytes.length > AttachmentLimits.total
      )
        throw new DocumentError(
          "rejected",
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
}
class HostAttachments implements AttachmentStore {
  async put(bytes: Uint8Array): Promise<AttachmentInfo> {
    checkSize(bytes.length);
    return call({ method: "attachments.put", bytes: base64.encode(bytes) });
  }
  async read(id: string) {
    assertAttachmentID(id);
    return base64.decode((await call({ method: "attachments.read", attachmentID: id })).bytes);
  }
}

export function ownerAttachments(
  doc: OwnerDocument<any>,
  native: boolean,
): SlopContext["attachments"] {
  const store = native ? new HostAttachments() : new MemoryAttachments();
  return {
    /**
     * Stores `file`, then runs `reference` as a synchronous collector (the rules of
     * `change()`) to write the reference. Resolves with the reference once both are
     * accepted; rejects with the write's error otherwise.
     */
    import(file, reference) {
      if (
        file.size > AttachmentLimits.file ||
        !file.name ||
        new TextEncoder().encode(file.name).length > AttachmentLimits.name ||
        new TextEncoder().encode(file.type).length > AttachmentLimits.name
      )
        return Promise.reject(new Error("Invalid attachment metadata or size"));
      return doc.admit(async () => {
        const saved = await store.put(new Uint8Array(await file.arrayBuffer()));
        return { ...saved, name: file.name, mimeType: file.type || "application/octet-stream" };
      }, reference);
    },
    async read(id: string, options: { type?: string } = {}) {
      return new Blob([(await store.read(id)) as Uint8Array<ArrayBuffer>], {
        type: options.type ?? "",
      });
    },
  };
}
