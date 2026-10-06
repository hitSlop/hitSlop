import { Strict } from "./strict";
import * as Type from "typebox";
import { validate } from "./validation";
import { AppLimits, ManifestText, ShapeLimits, SlopCategories, WindowBounds } from "./constants";
import { ThemeValuesSchema } from "./values";

const SlopCategorySchema = Type.Enum(SlopCategories, { title: "SlopCategory" });
const categoryBounds = { minItems: 1, maxItems: 2, uniqueItems: true };
const categories = Type.Array(SlopCategorySchema, categoryBounds);
const SlopAuthorSchema = Strict(
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
  maxLength: AppLimits.assetPath,
  pattern: "^(?!.*(?:^|/)\\.{1,2}(?:/|$)|.*//)assets/[A-Za-z0-9._/-]+\\.[pP][nN][gG]$",
});
const dimensions = {
  width: Type.Integer({ minimum: WindowBounds.minWidth, maximum: WindowBounds.max }),
  height: Type.Integer({ minimum: WindowBounds.minHeight, maximum: WindowBounds.max }),
};
const SlopPathShapeSchema = Strict(
  {
    path: Type.String({ minLength: 1, maxLength: ShapeLimits.path }),
    viewBox: Type.Optional(
      Type.Tuple([
        Type.Number({ minimum: 1, maximum: ShapeLimits.viewBox }),
        Type.Number({ minimum: 1, maximum: ShapeLimits.viewBox }),
      ]),
    ),
    fillRule: Type.Optional(Type.Enum(["nonzero", "evenodd"])),
  },
  { title: "SlopPathShape" },
);
// The core parses both forms (`crates/hitslop-core/src/shape.rs`); the schema bounds them.
const SlopShapeSchema = Type.Union(
  [
    Type.String({ minLength: 1, maxLength: ShapeLimits.radius }),
    SlopPathShapeSchema,
  ],
  { title: "SlopShape" },
);
const SlopStandardPresentationSchema = Strict(
  {
    ...dimensions,
    resizable: Type.Optional(Type.Boolean()),
    shape: Type.Optional(SlopShapeSchema),
    lockAspect: Type.Optional(Type.Boolean()),
    background: Type.Optional(Type.Enum(["transparent", "glass"])),
  },
  { title: "SlopStandardPresentation" },
);
const SlopSkinPresentationSchema = Strict(
  { ...dimensions, skin: skinPath },
  { title: "SlopSkinPresentation" },
);
export const SlopPresentationSchema = Type.Union(
  [SlopStandardPresentationSchema, SlopSkinPresentationSchema],
  { title: "SlopPresentation" },
);
const manifestFields = {
  author: SlopAuthorSchema,
  slug: Type.String({ ...ManifestText.slug }),
  title: Type.String({ ...ManifestText.title }),
  description: Type.String({ ...ManifestText.description }),
  categories,
  presentation: SlopPresentationSchema,
};
/** The manifest a `.slop` stores: `slop.ts`'s fields and its folder's name as the slug. */
export const SlopManifestSchema = Strict(manifestFields, { title: "SlopManifest" });
export type SlopCategory = Type.Static<typeof SlopCategorySchema>;
export type SlopManifest = Type.Static<typeof SlopManifestSchema>;
export type SlopPresentation = Type.Static<typeof SlopPresentationSchema>;
/** A window the host draws and the shape clips. */
export type SlopStandardPresentation = Type.Static<typeof SlopStandardPresentationSchema>;
/** A fixed-size window drawn by a PNG skin. */
export type SlopSkinPresentation = Type.Static<typeof SlopSkinPresentationSchema>;
/** A built app: what the file engine packs (a build's `app.json`) and a `.slop` stores in
 * its `app` row. The core checks each part by its own rules; only it interprets the
 * descriptor and the initial values. */
export const AppRowSchema = Strict(
  {
    packageFormat: Type.Integer({ minimum: 1 }),
    runtimeABI: Type.Integer({ minimum: 1 }),
    manifest: SlopManifestSchema,
    descriptor: Type.Unknown(),
    initial: Type.Unknown(),
    theme: ThemeValuesSchema,
  },
  { title: "AppRow" },
);
export type AppRow = Type.Static<typeof AppRowSchema>;
/** The authored manifest's structure. Window shape geometry is validated by the core (WASM). */
export const parseManifest = (input: unknown): SlopManifest =>
  validate(SlopManifestSchema, input, "Invalid manifest");
