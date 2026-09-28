import { HostAttachments, MemoryAttachments, attachmentLimits } from "../attachments";
import type { AttachmentRef } from "../contracts";
import type { OwnerDocument } from "./document";

export function ownerAttachments(doc: OwnerDocument<any>, native: boolean) {
  const store = native ? new HostAttachments() : new MemoryAttachments();
  return {
    store,
    async import(
      file: File,
      options: { commit(ref: AttachmentRef): void | Promise<void> },
    ): Promise<AttachmentRef> {
      if (
        file.size > attachmentLimits.file ||
        !file.name ||
        new TextEncoder().encode(file.name).length > 255 ||
        file.type.length > 255
      )
        throw new Error("Invalid attachment metadata or size");
      let ref!: AttachmentRef;
      await doc.stageSave(async (commit) => {
        const saved = await store.put(new Uint8Array(await file.arrayBuffer()));
        ref = { ...saved, name: file.name, mimeType: file.type || "application/octet-stream" };
        await commit(() => options.commit(ref));
      });
      return ref;
    },
    async read(id: string, options: { type?: string } = {}) {
      return new Blob([(await store.read(id)) as Uint8Array<ArrayBuffer>], {
        type: options.type ?? "",
      });
    },
    list: () => store.list(),
  };
}
