import { base64, call } from "./bridge";
import { AttachmentIdPattern, AttachmentLimits } from "../schema/constants";
import type { SlopContext } from "../sdk/abi";
import type { OwnerDocument } from "./owner/document";
import { preview } from "./preview";

const MiB = 1024 * 1024;
/** Why a file cannot become an attachment, said for the author, checked before it is read.
 * Photos and recordings arrive as Blobs: wrap one in a File to give it a name. */
function attachmentRefusal(file: Blob & { name?: unknown }) {
  if (file.size > AttachmentLimits.file)
    return `Attachment is ${(file.size / MiB).toFixed(1)} MiB; an attachment holds at most ${AttachmentLimits.file / MiB} MiB`;
  if (typeof file.name !== "string" || !file.name)
    return 'An attachment needs a file name: pass a File, such as new File([blob], "photo.jpg", { type: blob.type })';
  if (new TextEncoder().encode(file.name).length > AttachmentLimits.name)
    return `An attachment name holds at most ${AttachmentLimits.name} UTF-8 bytes`;
  return undefined;
}

export function ownerAttachments(
  doc: Pick<OwnerDocument<import("../sdk/schema").ObjectNode>, "admit">,
): SlopContext["attachments"] {
  const url = (id: string) => {
    if (!new RegExp(AttachmentIdPattern).test(id)) throw new Error("Invalid attachment ID");
    return preview.host
      ? preview.host.attachmentURL(id)
      : new URL(`/attachments/${id}`, location.href).href;
  };
  return {
    import(file, reference) {
      const refused = attachmentRefusal(file);
      if (refused) return Promise.reject(new Error(refused));
      return doc.admit(async () => {
        const saved = await call({
          method: "attachments.put",
          bytes: base64.encode(new Uint8Array(await file.arrayBuffer())),
        });
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
