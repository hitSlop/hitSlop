// Runtime values come from Rust, shared with the generated native constants.
export * from "../wire/constants.generated";
import { AttachmentIdRule } from "../wire/constants.generated";

export const AttachmentIdPattern = `^[${AttachmentIdRule.characters}]{${AttachmentIdRule.length}}$`;
