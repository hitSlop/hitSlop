import { defineDocument, s, type Value } from "@hitslop/document";

export const difficulties = ["Easy", "Medium", "Advanced"] as const;

const minutes = s.optional(s.number({ min: 0, max: 9_999 }));

const schema = defineDocument({
  title: s.text(),
  description: s.text(),
  difficulty: s.enum(difficulties),
  servings: s.optional(s.number({ min: 1, max: 999 })),
  prepMinutes: minutes,
  cookMinutes: minutes,
  ingredients: s.list(s.object({
    text: s.text(),
    checked: s.boolean(),
  })),
  steps: s.list(s.object({
    title: s.text(),
    text: s.text(),
    minutes,
  })),
  photo: s.optional(s.object({
    id: s.string(),
    name: s.string(),
    mimeType: s.string(),
  })),
});

export type Recipe = Value<typeof schema.fields.node>;
export type Ingredient = Recipe["ingredients"][number];
export type Step = Recipe["steps"][number];
export default schema;
