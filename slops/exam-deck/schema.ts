import { defineDocument, s } from "hitslop";

// SCHEMA.md explains every field and why it is stored. This shape is a long-term
// compatibility surface: after launch a newer build must open every released document.

const iso = (description: string) => s.string({ minLength: 20, maxLength: 32, description });

/** FSRS-6 weights as shipped by ts-fsrs 5.4.2 (`default_w`). Copied, never imported, so a
 * library upgrade cannot change what a saved document means. */
export const fsrs6DefaultWeights = [
  0.212, 1.2931, 2.3065, 8.2956, 6.4133, 0.8334, 3.0194, 0.001, 1.8722, 0.1666, 0.796,
  1.4835, 0.0614, 0.2629, 1.6483, 0.6014, 1.8729, 0.5425, 0.0912, 0.0658, 0.1542,
];

const sampleCards: [string, string][] = [
  ["What does the mitochondrion make most of?", "ATP, by oxidative phosphorylation on the inner membrane."],
  ["Where are ribosomes made?", "In the nucleolus, inside the nucleus."],
  ["What does the rough ER do?", "Folds and modifies proteins made by its bound ribosomes."],
  ["What does the smooth ER do?", "Makes lipids and steroids and detoxifies; stores calcium in muscle."],
  ["What does the Golgi apparatus do?", "Modifies, sorts and packages proteins into vesicles."],
  ["What is in a lysosome?", "Acid hydrolases that break down macromolecules and old organelles."],
  ["Name the phases of mitosis in order.", "Prophase, prometaphase, metaphase, anaphase, telophase."],
  ["When do sister chromatids separate?", "Anaphase (anaphase II in meiosis)."],
  ["When does crossing over happen?", "Prophase I of meiosis, between homologous chromosomes."],
  ["What is the fluid mosaic model?", "A phospholipid bilayer with proteins that move laterally within it."],
  ["Osmosis moves water toward…", "The side with the higher solute concentration (lower water potential)."],
  ["What does the sodium–potassium pump move?", "3 Na⁺ out and 2 K⁺ in per ATP."],
  ["Glycolysis: where, and net yield?", "Cytoplasm; 2 ATP, 2 NADH and 2 pyruvate per glucose."],
  ["Where does the Krebs cycle run?", "The mitochondrial matrix."],
  ["Final electron acceptor in aerobic respiration?", "Oxygen, reduced to water."],
  ["Light reactions take place in the…", "Thylakoid membranes of the chloroplast."],
  ["What does the Calvin cycle fix?", "CO₂ into G3P, using ATP and NADPH, in the stroma."],
  ["What enzyme fixes CO₂ in the Calvin cycle?", "RuBisCO."],
  ["Prokaryote vs eukaryote, in one line?", "Prokaryotes lack a nucleus and membrane-bound organelles."],
  ["What does a G1 checkpoint check?", "Cell size, nutrients, growth signals and DNA damage before S phase."],
  ["What are cyclins?", "Proteins whose levels rise and fall to activate CDKs through the cell cycle."],
  ["What does p53 do?", "Halts the cycle to repair DNA damage, or triggers apoptosis."],
  ["Diffusion vs facilitated diffusion?", "Both go down a gradient; facilitated needs a channel or carrier protein."],
  ["What is endocytosis?", "Taking material in by folding the membrane into a vesicle."],
];

export default defineDocument({
  title: s.string({ minLength: 1, maxLength: 80, description: "The deck's name, shown in the window." }),
  examDate: s.optional(s.string({
    minLength: 10, maxLength: 10,
    description: "The exam's local calendar date, YYYY-MM-DD. Absent until the person sets one.",
  })),
  dailyMinutes: s.integer({ min: 1, max: 600, description: "Study budget per day, in minutes." }),
  secondsPerCard: s.integer({ min: 3, max: 600, description: "Assumed time per review; the daily card cap is dailyMinutes × 60 / secondsPerCard." }),
  newPerDay: s.optional(s.integer({
    min: 1, max: 1000,
    description: "New cards introduced per day. Absent means spread the remaining new cards evenly until two days before the exam.",
  })),
  scheduler: s.object({
    algorithm: s.enum(["fsrs-6"], { description: "The scheduling model these parameters and the card states belong to." }),
    w: s.list(s.number(), { description: "The 21 FSRS-6 weights, exactly as the scheduler uses them." }),
    requestRetention: s.number({ min: 0.7, max: 0.99, description: "Target probability of recall when a card comes due." }),
    maximumInterval: s.integer({ min: 1, max: 36500, description: "Longest interval in days." }),
    enableFuzz: s.boolean({ description: "Interval fuzz. The app writes false so schedules and forecasts are reproducible." }),
    enableShortTerm: s.boolean({ description: "Use same-day learning and relearning steps." }),
    learningSteps: s.list(s.string({ minLength: 2, maxLength: 8 }), { description: "Same-day steps for new cards, like \"1m\", \"10m\" (m, h or d)." }),
    relearningSteps: s.list(s.string({ minLength: 2, maxLength: 8 }), { description: "Same-day steps after a lapse." }),
  }, { description: "The algorithm parameters every card state and review was scheduled with." }),
  cards: s.list(s.object({
    front: s.text({ description: "The prompt." }),
    back: s.text({ description: "The answer." }),
    state: s.enum(["new", "learning", "review", "relearning"], { default: "new", description: "FSRS card state (ts-fsrs State 0–3)." }),
    due: s.optional(iso("When the card is next due, ISO 8601 UTC. Absent while the card is new.")),
    stability: s.number({ min: 0, default: 0, description: "FSRS stability in days (0 while new)." }),
    difficulty: s.number({ min: 0, max: 10, default: 0, description: "FSRS difficulty, 1–10 (0 while new)." }),
    scheduledDays: s.integer({ min: 0, default: 0, description: "The interval in days the last review chose (0 during same-day steps)." }),
    learningStep: s.integer({ min: 0, default: 0, description: "Index into the current learning or relearning steps (ts-fsrs learning_steps)." }),
    reps: s.integer({ min: 0, default: 0, description: "Reviews of this card, including same-day steps." }),
    lapses: s.integer({ min: 0, default: 0, description: "Times the card was forgotten after graduating." }),
    lastReview: s.optional(iso("When the card was last reviewed, ISO 8601 UTC. Absent while the card is new.")),
  }), { description: "The deck, in the order new cards are introduced." }),
  reviews: s.list(s.object({
    card: s.string({ minLength: 1, maxLength: 64, description: "The reviewed card's $id." }),
    rating: s.integer({ min: 1, max: 4, description: "1 Again, 2 Hard, 3 Good, 4 Easy." }),
    at: iso("When the review happened, ISO 8601 UTC."),
  }), { description: "Append-only review log, oldest first. See SCHEMA.md for its growth policy." }),
}, { initial: {
  title: "Cell biology midterm",
  dailyMinutes: 20,
  secondsPerCard: 20,
  newPerDay: 8,
  scheduler: {
    algorithm: "fsrs-6",
    w: fsrs6DefaultWeights,
    requestRetention: 0.9,
    maximumInterval: 36500,
    enableFuzz: false,
    enableShortTerm: true,
    learningSteps: ["1m", "10m"],
    relearningSteps: ["10m"],
  },
  cards: sampleCards.map(([front, back]) => ({ front, back })),
  reviews: [],
} });
