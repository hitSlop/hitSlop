import { defineDocument, s } from "@hitslop/tinys";

export const difficulties = ["Easy", "Medium", "Advanced"] as const;

const minutes = s.optional(s.number({ min: 0, max: 9_999 }));

export default defineDocument({
  title: s.text(),
  description: s.text(),
  difficulty: s.enum(difficulties, { default: "Easy" }),
  servings: s.optional(s.number({ min: 1, max: 999 })),
  prepMinutes: minutes,
  cookMinutes: minutes,
  ingredients: s.list(s.object({
    text: s.text(),
    checked: s.boolean({ default: false }),
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
}, { initial: {
  title: "Lemon garlic pasta",
  description: "Silky, bright, and ready before the table is set.",
  difficulty: "Easy",
  servings: 2,
  prepMinutes: 10,
  cookMinutes: 20,
  ingredients: [
    { text: "200g spaghetti" },
    { text: "4 garlic cloves, thinly sliced" },
    { text: "2 tbsp extra-virgin olive oil" },
    { text: "1 lemon, zest and juice" },
    { text: "½ cup finely grated parmesan" },
    { text: "Salt, pepper, and parsley" },
  ],
  steps: [
    { title: "Boil the pasta", text: "Cook the pasta in well-salted water until just al dente.", minutes: 10 },
    { title: "Sizzle the garlic", text: "Gently sizzle the garlic in olive oil until fragrant.", minutes: 3 },
    { title: "Build the sauce", text: "Add pasta, lemon zest, juice, and a splash of pasta water.", minutes: 2 },
    { title: "Finish and serve", text: "Toss with parmesan until glossy. Season and serve." },
  ],
} });
