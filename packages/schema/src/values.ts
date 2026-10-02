import * as T from "typebox";
import { AttachmentIdPattern, AttachmentLimits, ManifestText, ThemeTokenRule, base64Length } from "./constants";
import { Strict } from "./strict";
export const AttachmentIDSchema = T.String({
  pattern: AttachmentIdPattern,
  minLength: 64,
  maxLength: 64,
});
export const AttachmentBytesSchema = T.String({ maxLength: base64Length(AttachmentLimits.file) });
export const ThemeTokenSchema = T.String({ pattern: ThemeTokenRule.name });
/** A palette: declared token names and their colors. */
export const ThemeValuesSchema = T.Record(ThemeTokenSchema, T.String({ pattern: ThemeTokenRule.value }), {
  additionalProperties: false,
  maxProperties: ThemeTokenRule.tokens,
});
/** A shared theme file: the template it was made for and its full palette. */
export const ThemeFileSchema = Strict({
  template: T.String({ ...ManifestText.slug }),
  values: ThemeValuesSchema,
});
export type ThemeFile = T.Static<typeof ThemeFileSchema>;
export const AttachmentInfoSchema = Strict({
  id: AttachmentIDSchema,
  byteLength: T.Integer({ minimum: 0 }),
});
export type AttachmentInfo = T.Static<typeof AttachmentInfoSchema>;
