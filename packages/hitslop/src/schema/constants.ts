// Runtime values come from Rust, shared with the generated native constants.
export * from "../wire/constants.generated";
import { AttachmentIdRule } from "../wire/constants.generated";

export const base64Length = (bytes: number) => 4 * Math.ceil(bytes / 3);
export const AttachmentIdPattern = `^[${AttachmentIdRule.characters}]{${AttachmentIdRule.length}}$`;
