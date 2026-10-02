import { Strict } from "./strict";
import * as Type from "typebox";
import { validate } from "./validation";
import { ManifestText, PackageFormat, RuntimeABI, SlopCategories, WindowBounds } from "./constants";

export const manifestSchemaURL = "https://api.hitslop.com/schemas/manifest.schema.json" as const;
export const SlopCategorySchema = Type.Enum(SlopCategories, { title: "SlopCategory" });
const categoryBounds = { minItems: 1, maxItems: 2, uniqueItems: true };
const categories = Type.Array(SlopCategorySchema, categoryBounds);
export const SlopAuthorSchema = Strict(
  {
    name: Type.String({ ...ManifestText.authorName }),
    url: Type.Optional(
      Type.String({ maxLength: 2048, format: "uri", pattern: "^https?://[^/?#\\s]+" }),
    ),
  },
  { title: "SlopAuthor" },
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
export const SlopPathShapeSchema = Strict(
  {
    path: Type.String({ minLength: 1, maxLength: 4096 }),
    viewBox: Type.Optional(
      Type.Tuple([
        Type.Number({ minimum: 1, maximum: 16384 }),
        Type.Number({ minimum: 1, maximum: 16384 }),
      ]),
    ),
    fillRule: Type.Optional(Type.Enum(["nonzero", "evenodd"])),
  },
  { title: "SlopPathShape" },
);
const radiusValue = "(?:0|(?:[0-9]+(?:\\.[0-9]+)?|\\.[0-9]+)(?:px|%))";
const radiusList = `${radiusValue}(?:[ \t\r\n]+${radiusValue}){0,3}`;
export const SlopShapeSchema = Type.Union(
  [
    Type.String({
      minLength: 1,
      maxLength: 256,
      pattern: `^[ \t\r\n]*${radiusList}(?:[ \t\r\n]*/[ \t\r\n]*${radiusList})?[ \t\r\n]*$`,
    }),
    SlopPathShapeSchema,
  ],
  { title: "SlopShape" },
);
export const SlopStandardPresentationSchema = Strict(
  {
    ...dimensions,
    resizable: Type.Optional(Type.Boolean()),
    shape: Type.Optional(SlopShapeSchema),
    lockAspect: Type.Optional(Type.Boolean()),
    background: Type.Optional(Type.Literal("transparent")),
  },
  { title: "SlopStandardPresentation" },
);
export const SlopSkinPresentationSchema = Strict(
  { ...dimensions, skin: skinPath },
  { title: "SlopSkinPresentation" },
);
export const SlopPresentationSchema = Type.Union(
  [SlopStandardPresentationSchema, SlopSkinPresentationSchema],
  { title: "SlopPresentation" },
);
const manifestFields = {
  $schema: Type.Optional(Type.Literal(manifestSchemaURL)),
  author: SlopAuthorSchema,
  slug: Type.String({ ...ManifestText.slug }),
  title: Type.String({ ...ManifestText.title }),
  description: Type.String({ ...ManifestText.description }),
  categories,
  presentation: SlopPresentationSchema,
};
/** The manifest an author writes. */
export const SlopManifestSchema = Strict(manifestFields, { title: "SlopManifest" });
/** Independent package syntax and app runtime requirements, stamped by the builder. */
export const SlopPackageManifestSchema = Strict(
  { ...manifestFields, packageFormat: Type.Integer({ minimum: 1, maximum: PackageFormat }), runtimeABI: Type.Integer({ minimum: 1, maximum: RuntimeABI }) },
  { title: "SlopPackageManifest" },
);
export type SlopCategory = Type.Static<typeof SlopCategorySchema>;
export type SlopAuthor = Type.Static<typeof SlopAuthorSchema>;
export type SlopManifest = Type.Static<typeof SlopManifestSchema>;
export type SlopPackageManifest = Type.Static<typeof SlopPackageManifestSchema>;
export type SlopPresentation = Type.Static<typeof SlopPresentationSchema>;
/** The authored manifest's structure. Window shape geometry is validated by the core (WASM). */
export const parseManifest = (input: unknown): SlopManifest =>
  validate(SlopManifestSchema, input, "Invalid manifest");
/** A built package's manifest, at a level this build supports. */
export const parsePackageManifest = (input: unknown): SlopPackageManifest =>
  validate(SlopPackageManifestSchema, input, "Invalid package manifest");
