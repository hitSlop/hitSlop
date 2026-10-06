import type { Input } from "@hitslop/document";
import schema, { type Slot } from "./schema";

const list = (slot: Slot, ...texts: string[]) => texts.map((text) => ({ slot, text }));

export default {
  title: "",
  loops: 0,
  options: [
    ...list("home", "Mansion", "Apartment", "Shack", "House"),
    ...list("spouse", "Timothée", "Zendaya", "My crush", "Nobody (queen)"),
    ...list("kids", "No kids", "2 kids", "Twins", "7 kids"),
    ...list("car", "Tesla", "Beetle", "Minivan", "Skateboard"),
    ...list("job", "Pop star", "Doctor", "Astronaut", "Barista"),
    ...list("city", "Paris", "Tokyo", "Los Angeles", "Home town"),
    ...list("pet", "Dog", "Cat", "Capybara", "Dragon"),
  ],
} satisfies Input<typeof schema.descriptor>;
