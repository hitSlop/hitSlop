import type { Input } from "@hitslop/document";
import schema from "./schema";

export default {
  title: "Lemon garlic pasta",
  description: "Silky, bright, and ready before the table is set.",
  difficulty: "Easy",
  servings: 2,
  prepMinutes: 10,
  cookMinutes: 20,
  ingredients: [
    { text: "200g spaghetti", checked: false },
    { text: "4 garlic cloves, thinly sliced", checked: false },
    { text: "2 tbsp extra-virgin olive oil", checked: false },
    { text: "1 lemon, zest and juice", checked: false },
    { text: "½ cup finely grated parmesan", checked: false },
    { text: "Salt, pepper, and parsley", checked: false },
  ],
  steps: [
    { title: "Boil the pasta", text: "Cook the pasta in well-salted water until just al dente.", minutes: 10 },
    { title: "Sizzle the garlic", text: "Gently sizzle the garlic in olive oil until fragrant.", minutes: 3 },
    { title: "Build the sauce", text: "Add pasta, lemon zest, juice, and a splash of pasta water.", minutes: 2 },
    { title: "Finish and serve", text: "Toss with parmesan until glossy. Season and serve." },
  ],
} satisfies Input<typeof schema.fields.node>;
