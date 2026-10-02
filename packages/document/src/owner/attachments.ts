import { HostAttachments, MemoryAttachments, attachmentLimits } from "../attachments";
import type { AttachmentRef } from "../abi";
import type { OwnerDocument } from "./document";
import type { Scope as OwnerScope } from "../abi";

export function ownerAttachments(doc: OwnerDocument<any>, native: boolean) {
  const store = native ? new HostAttachments() : new MemoryAttachments();
  return {
    store,
    /**
     * Stores `file`, then runs `reference` as a synchronous collector (the rules of
     * `change()`) to write the reference. Resolves with the reference once both are
     * accepted; rejects with the write's error otherwise.
     */
    import(file: File, reference: (tx: OwnerScope<any>, ref: AttachmentRef) => void): Promise<AttachmentRef> {
      if (
        file.size > attachmentLimits.file ||
        !file.name ||
        new TextEncoder().encode(file.name).length > attachmentLimits.name ||
        new TextEncoder().encode(file.type).length > attachmentLimits.name
      )
        return Promise.reject(new Error("Invalid attachment metadata or size"));
      return doc.admit(async () => {
        const saved = await store.put(new Uint8Array(await file.arrayBuffer()));
        return { ...saved, name: file.name, mimeType: file.type || "application/octet-stream" };
      }, reference);
    },
    async read(id: string, options: { type?: string } = {}) {
      return new Blob([(await store.read(id)) as Uint8Array<ArrayBuffer>], { type: options.type ?? "" });
    },
  };
}
