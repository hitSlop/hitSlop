import p5 from "p5";

// p5.brush registers its p5 addon against a global p5 when its module evaluates,
// so this module must be imported before "p5.brush".
(globalThis as typeof globalThis & { p5?: typeof p5 }).p5 = p5;
p5.disableFriendlyErrors = true;
export default p5;
