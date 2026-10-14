import { defineDocument, s } from "hitslop";

export const categories = ["Produce", "Dairy", "Bakery", "Pantry", "Household", "Other"] as const;

export default defineDocument({
  title: s.text(),
  stickyNote: s.text(),
  items: s.list(s.object({
    text: s.text(),
    category: s.enum(categories),
    done: s.boolean({ default: false }),
  })),
}, { initial: {
  title: "Groceries",
  stickyNote: "Don't forget coffee!",
  items: [
    { text: "Organic whole milk", category: "Dairy", done: true },
    { text: "Pasture eggs (dozen)", category: "Dairy", done: true },
    { text: "Sourdough loaf", category: "Bakery" },
    { text: "Hass avocados", category: "Produce" },
    { text: "Cherry tomatoes", category: "Produce" },
    { text: "Baby spinach", category: "Produce" },
    { text: "Oat barista milk", category: "Dairy" },
    { text: "Olive oil", category: "Pantry" },
  ],
} });
