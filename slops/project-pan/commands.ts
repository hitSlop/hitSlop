import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc, { kinds } from "./schema";
import { addProduct, removeProduct, setLeft, setOpened, setPao, setShade } from "./pan";

const product = s.string({ description: "The product's $id." });

export default defineCommands(doc, {
  addProduct: {
    description: "Add a full product to the tray and return its stable row ID.",
    args: {
      name: s.string({ description: "The product name." }),
      brand: s.optional(s.string({ description: "The brand." })),
      kind: s.optional(s.enum(kinds, { description: "The kind of product; defaults to face." })),
      shade: s.optional(s.string({ description: "A #rrggbb shade; defaults to the next swatch." })),
      pao: s.optional(s.integer({ min: 1, max: 60, description: "Months it stays good once opened; defaults to 12." })),
    },
    run: ({ tx, current }, args) => addProduct(tx, args, current.products.length),
  },
  setLeft: {
    description: "Set how much of a product is left, 0 to 100 percent; 0 moves it to the empties shelf.",
    args: { product, left: s.integer({ min: 0, max: 100, description: "Percent left." }) },
    run: ({ tx }, args) => setLeft(tx, args.product, args.left),
  },
  setOpened: {
    description: "Set the date a product was opened (YYYY-MM-DD), or an empty string to clear it.",
    args: { product, date: s.string({ description: "A YYYY-MM-DD date, or an empty string to clear it." }) },
    run: ({ tx }, args) => setOpened(tx, args.product, args.date),
  },
  setPao: {
    description: "Set a product's period after opening in months.",
    args: { product, months: s.integer({ min: 1, max: 60, description: "Months, 1 to 60." }) },
    run: ({ tx }, args) => setPao(tx, args.product, args.months),
  },
  setShade: {
    description: "Set the shade a product's pan shows.",
    args: { product, shade: s.string({ description: "A #rrggbb colour." }) },
    run: ({ tx }, args) => setShade(tx, args.product, args.shade),
  },
  removeProduct: {
    description: "Remove a product from the compact.",
    args: { product },
    run: ({ tx }, args) => removeProduct(tx, args.product),
  },
});
