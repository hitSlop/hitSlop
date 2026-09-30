import * as Type from "typebox";
import { validate } from "./validation";
import { ManifestText, SlopCategories, WindowBounds } from "./constants";

export const manifestSchemaURL = "https://api.hitslop.com/schemas/manifest.schema.json" as const;
export const SlopCategorySchema = Type.Enum(SlopCategories, { title: "SlopCategory" });
const categoryBounds = { minItems: 1, maxItems: 2, uniqueItems: true };
const categories = Type.Array(SlopCategorySchema, categoryBounds);
export const SlopAuthorSchema = Type.Object(
  {
    name: Type.String({ ...ManifestText.authorName }),
    url: Type.Optional(
      Type.String({ maxLength: 2048, format: "uri", pattern: "^https?://[^/?#\\s]+" }),
    ),
  },
  { additionalProperties: false, title: "SlopAuthor" },
);
const skinPath = Type.String({
  minLength: 12,
  maxLength: 240,
  pattern: "^(?!.*(?:^|/)\\.{1,2}(?:/|$)|.*//)assets/[A-Za-z0-9._/-]+\\.[pP][nN][gG]$",
});
const dimensions = {
  width: Type.Integer({ minimum: WindowBounds.minWidth, maximum: WindowBounds.max }),
  height: Type.Integer({ minimum: WindowBounds.minHeight, maximum: WindowBounds.max }),
};
export const SlopPathShapeSchema = Type.Object({
  path: Type.String({ minLength: 1, maxLength: 4096 }),
  viewBox: Type.Optional(Type.Tuple([Type.Number({ minimum: 1, maximum: 16384 }), Type.Number({ minimum: 1, maximum: 16384 })])),
  fillRule: Type.Optional(Type.Enum(["nonzero", "evenodd"])),
}, { additionalProperties: false, title: "SlopPathShape" });
const radiusValue = "(?:0|(?:[0-9]+(?:\\.[0-9]+)?|\\.[0-9]+)(?:px|%))";
const radiusList = `${radiusValue}(?:[ \t\r\n]+${radiusValue}){0,3}`;
export const SlopShapeSchema = Type.Union([
  Type.String({ minLength: 1, maxLength: 256, pattern: `^[ \t\r\n]*${radiusList}(?:[ \t\r\n]*/[ \t\r\n]*${radiusList})?[ \t\r\n]*$` }), SlopPathShapeSchema,
], { title: "SlopShape" });
export const SlopStandardPresentationSchema = Type.Object(
  {
    ...dimensions,
    resizable: Type.Optional(Type.Boolean()),
    shape: Type.Optional(SlopShapeSchema),
    lockAspect: Type.Optional(Type.Boolean()),
    background: Type.Optional(Type.Literal("transparent")),
  },
  { additionalProperties: false, title: "SlopStandardPresentation" },
);
export const SlopSkinPresentationSchema = Type.Object(
  { ...dimensions, skin: skinPath },
  { additionalProperties: false, title: "SlopSkinPresentation" },
);
export const SlopPresentationSchema = Type.Union(
  [SlopStandardPresentationSchema, SlopSkinPresentationSchema],
  { title: "SlopPresentation" },
);
export const SlopManifestSchema = Type.Object(
  {
    $schema: Type.Optional(Type.Literal(manifestSchemaURL)),
    author: SlopAuthorSchema,
    slug: Type.String({ ...ManifestText.slug }),
    title: Type.String({ ...ManifestText.title }),
    description: Type.String({ ...ManifestText.description }),
    categories,
    presentation: SlopPresentationSchema,
  },
  { additionalProperties: false, title: "SlopManifest" },
);
export type SlopCategory = Type.Static<typeof SlopCategorySchema>;
export type SlopAuthor = Type.Static<typeof SlopAuthorSchema>;
export type SlopManifest = Type.Static<typeof SlopManifestSchema>;
export type SlopPresentation = Type.Static<typeof SlopPresentationSchema>;
/** The manifest's structure. Window shape geometry is validated by the core (WASM). */
export const parseManifest = (input: unknown): SlopManifest => validate(SlopManifestSchema, input, "Invalid manifest");
