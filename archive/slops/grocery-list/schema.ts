import { defineDocument, s, type Value } from "@hitslop/document";

export const categories = ["Produce", "Dairy", "Bakery", "Pantry", "Household", "Other"] as const;

const schema = defineDocument({
  title: s.text(),
  stickyNote: s.text(),
  items: s.list(s.object({
    text: s.text(),
    category: s.enum(categories),
    done: s.boolean(),
  })),
});

export type GroceryList = Value<typeof schema.fields.node>;
export type GroceryItem = GroceryList["items"][number];
export default schema;
