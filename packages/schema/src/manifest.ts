import * as Type from "typebox";
import { Check } from "typebox/value";
function validate<S extends Type.TSchema>(schema: S, input: unknown): Type.Static<S> {
  if (!Check(schema, input)) throw new Error("Invalid v1 manifest");
  return input as Type.Static<S>;
}

export const manifestSchemaURL = "https://api.hitslop.com/schemas/v1/manifest.schema.json" as const;
export const SlopCategorySchema = Type.Enum(
  [
    "productivity",
    "utilities",
    "finance",
    "media",
    "games",
    "developer-tools",
    "education",
    "business",
    "personal",
    "other",
  ],
  { title: "SlopCategory" },
);
const categoryBounds = { minItems: 1, maxItems: 2, uniqueItems: true };
const categories = Type.Array(SlopCategorySchema, categoryBounds);
export const SlopAuthorSchema = Type.Object(
  {
    name: Type.String({ minLength: 1, maxLength: 80, pattern: "\\S" }),
    url: Type.Optional(
      Type.String({ maxLength: 2048, format: "uri", pattern: "^https?://[^/?#\\s]+" }),
    ),
  },
  { additionalProperties: false, title: "SlopAuthor" },
);
export const relativePath = Type.String({
  minLength: 1,
  maxLength: 240,
  pattern: "^(?!/)(?!.*(?:^|/)\\.\\.(?:/|$))[A-Za-z0-9._-]+(?:/[A-Za-z0-9._-]+)*$",
});
const skinPath = Type.String({
  minLength: 12,
  maxLength: 240,
  pattern: "^assets/(?!.*(?:^|/)\\.\\.(?:/|$))[A-Za-z0-9._/-]+\\.[pP][nN][gG]$",
});
const dimensions = {
  width: Type.Integer({ minimum: 240, maximum: 4096 }),
  height: Type.Integer({ minimum: 180, maximum: 4096 }),
};
export const SlopStandardPresentationSchema = Type.Object(
  {
    ...dimensions,
    resizable: Type.Optional(Type.Boolean()),
    shape: Type.Optional(Type.Enum(["rounded", "ellipse", "capsule"])),
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
    slug: Type.String({ minLength: 2, maxLength: 64, pattern: "^[a-z0-9]+(?:-[a-z0-9]+)*$" }),
    title: Type.String({ minLength: 1, maxLength: 80 }),
    description: Type.String({ minLength: 1, maxLength: 240 }),
    categories,
    presentation: SlopPresentationSchema,
  },
  { additionalProperties: false, title: "SlopManifest" },
);
export type SlopCategory = Type.Static<typeof SlopCategorySchema>;
export type SlopAuthor = Type.Static<typeof SlopAuthorSchema>;
export type SlopManifest = Type.Static<typeof SlopManifestSchema>;
export type SlopPresentation = Type.Static<typeof SlopPresentationSchema>;
export const parseManifest = (input: unknown): SlopManifest => validate(SlopManifestSchema, input);

// Readers accept metadata additions, but known fields and variant boundaries stay strict.
// Referencing the writer's fields preserves every existing constraint without transforming unions.
const futureEnum = Type.String({ minLength: 1, maxLength: 64 });
const absent = Type.Optional(Type.Never());
export const SlopManifestReadSchema = Type.Object({
  ...SlopManifestSchema.properties,
  author: Type.Object(SlopAuthorSchema.properties),
  categories: Type.Array(futureEnum, categoryBounds),
  presentation: Type.Union([
    Type.Object({
      ...SlopStandardPresentationSchema.properties,
      shape: Type.Optional(futureEnum),
      background: Type.Optional(futureEnum),
      skin: absent,
    }),
    Type.Object({
      ...SlopSkinPresentationSchema.properties,
      resizable: absent,
      shape: absent,
      background: absent,
    }),
  ]),
});
