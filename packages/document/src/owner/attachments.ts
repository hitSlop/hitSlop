import { HostAttachments, MemoryAttachments, attachmentLimits } from "../attachments";
import type { AttachmentRef } from "../contracts";
import type { OwnerDocument, OwnerScope } from "./document";

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
        new TextEncoder().encode(file.name).length > 255 ||
        file.type.length > 255
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
    list: () => store.list(),
  };
}
