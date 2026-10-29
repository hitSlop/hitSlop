import { defineDocument, s } from "@hitslop/tinys";

export const slots = ["home", "spouse", "kids", "car", "job", "city", "pet"] as const;

const list = (slot: (typeof slots)[number], ...texts: string[]) => texts.map((text) => ({ slot, text }));

export default defineDocument({
  title: s.text(),
  // How many loops the last spiral had; 0 means it has not been drawn yet.
  loops: s.integer({ min: 0, max: 20, default: 0 }),
  options: s.list(s.object({
    slot: s.enum(slots),
    text: s.text(),
  })),
}, { initial: {
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
} });
