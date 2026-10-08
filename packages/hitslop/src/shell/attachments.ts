import { base64, call } from "./bridge";
import { AttachmentIdPattern, AttachmentLimits } from "../schema/constants";
import type { SlopContext } from "../sdk/abi";
import type { OwnerDocument } from "./owner/document";
import { preview } from "./preview";

export function ownerAttachments(doc: Pick<OwnerDocument<import("../sdk/schema").ObjectNode>, "admit">): SlopContext["attachments"] {
  const url = (id: string) => {
    if (!new RegExp(AttachmentIdPattern).test(id)) throw new Error("Invalid attachment ID");
    return preview.host ? preview.host.attachmentURL(id) : new URL(`/attachments/${id}`, location.href).href;
  };
  return {
    import(file, reference) {
      if (file.size > AttachmentLimits.file || !file.name || new TextEncoder().encode(file.name).length > AttachmentLimits.name)
        return Promise.reject(new Error("Invalid attachment metadata or size"));
      return doc.admit(async () => {
        const saved = await call({ method: "attachments.put", bytes: base64.encode(new Uint8Array(await file.arrayBuffer())) });
        return { ...saved, name: file.name };
      }, reference);
    },
    url,
    async read(id) {
      const response = await fetch(url(id));
      if (!response.ok) throw new Error("Attachment not found or damaged");
      return response.blob();
    },
  };
}
