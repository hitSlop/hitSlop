import { defineDocument, s } from "@hitslop/tinys";

const bpm = (description: string, value: number) =>
  s.integer({ min: 20, max: 300, default: value, description });

const sheet = defineDocument({
  abc: s.text({ description: "The piece in ABC notation; the score is engraved from it." }),
  tempo: bpm("The tempo you practise at now, in beats per minute.", 84),
  ladder: s.object({
    start: bpm("The tempo the ladder starts from.", 72),
    goal: bpm("The tempo you are working towards.", 112),
    step: s.integer({ min: 1, max: 40, default: 4, description: "Beats per minute added after each clean run." }),
  }, { description: "The tempo ladder: start, goal and the step after each clean run." }),
  log: s.list(s.object({
    at: s.string({ minLength: 10, maxLength: 10, description: "The day practised, YYYY-MM-DD in local time." }),
    tempo: bpm("The tempo reached that session.", 84),
    minutes: s.integer({ min: 1, max: 600, description: "Minutes practised." }),
  }), { description: "One row per logged practice session." }),
}, {
  initial: {
    abc: [
      "X:1",
      "T:Minuet in G",
      "C:attr. Christian Petzold",
      "M:3/4",
      "L:1/8",
      "K:G",
      "|: d2 GA Bc | d2 G2 G2 | e2 cd ef | g2 G2 G2 |",
      "c2 dc BA | B2 cB AG | F2 GA BG | A6 |",
      "d2 GA Bc | d2 G2 G2 | e2 cd ef | g2 G2 G2 |",
      "c2 dc BA | B2 cB AG | A2 BA GF | G6 :|",
    ].join("\n"),
    tempo: 84,
    ladder: { start: 72, goal: 112, step: 4 },
    log: [
      { at: "2026-10-05", tempo: 72, minutes: 15 },
      { at: "2026-10-06", tempo: 76, minutes: 20 },
      { at: "2026-10-07", tempo: 76, minutes: 10 },
      { at: "2026-10-08", tempo: 80, minutes: 25 },
      { at: "2026-10-09", tempo: 84, minutes: 20 },
    ],
  },
});

export default sheet;
