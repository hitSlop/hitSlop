import * as T from "typebox";
import { AttachmentIdPattern, AttachmentLimits, ManifestText, OutcomeCodes, ThemeTokenRule, base64Length } from "./constants";
import { Strict } from "./strict";
export const AttachmentIDSchema = T.String({
  pattern: AttachmentIdPattern,
  minLength: 64,
  maxLength: 64,
});
export const AttachmentBytesSchema = T.String({ maxLength: base64Length(AttachmentLimits.file) });
// The core owns the palette rules (`crates/hitslop-core/src/theme.rs`); the schema bounds
// a palette's shape.
export const ThemeTokenSchema = T.String({ minLength: 1, maxLength: ThemeTokenRule.nameLength });
/** A palette: declared token names and their colors. */
export const ThemeValuesSchema = T.Record(ThemeTokenSchema, T.String({ maxLength: 9 }), {
  additionalProperties: false,
  maxProperties: ThemeTokenRule.tokens,
});
/** Palette changes: each token's new color, or `null` to return it to the template's. */
export const ThemeChangesSchema = T.Record(ThemeTokenSchema, T.Union([T.String({ maxLength: 9 }), T.Null()]), {
  additionalProperties: false,
  maxProperties: ThemeTokenRule.tokens,
});
/** A shared theme file: the template it was made for and its full palette. */
export const ThemeFileSchema = Strict({
  template: T.String({ ...ManifestText.slug }),
  values: ThemeValuesSchema,
});
export const OutcomeCodeSchema = T.Enum(OutcomeCodes, { title: "OutcomeCode" });
export type OutcomeCode = T.Static<typeof OutcomeCodeSchema>;
/** A palette as the core reports it: the template's colors, the overrides and the result. */
export const ThemeStateSchema = Strict({ defaults: ThemeValuesSchema, overrides: ThemeValuesSchema, effective: ThemeValuesSchema });
export const AttachmentInfoSchema = Strict({
  id: AttachmentIDSchema,
  byteLength: T.Integer({ minimum: 0 }),
});
export type AttachmentInfo = T.Static<typeof AttachmentInfoSchema>;
