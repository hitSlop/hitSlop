// A deliberately small LaTeX-to-function compiler for plotting. Pure: no DOM, clock or
// randomness. Anything outside the set below is refused with a plain reason, and the
// page shows "Can't plot this" instead of guessing.
//
// Plottable: `y = …`, `f(x) = …` (any letter), `… = y`, a bare expression in x, or
// `left = right` (plotted as left − right, whose zeros are the solutions).
// Inside: numbers, x, the constants e and \pi, single-letter parameters (any letter but
// x, y, e), + − \cdot \times / \div, implicit multiplication, ^ powers, \frac, \sqrt,
// \sqrt[n], ( ) [ ] \left \right, |…|, and \sin \cos \tan \sec \csc \cot \arcsin \arccos
// \arctan \sinh \cosh \tanh \ln \log \log_b \exp (\sin^{-1} reads as \arcsin).

export type Params = Readonly<Record<string, number>>;
export type Evaluate = (x: number, params: Params) => number;
export type Plot =
  | { ok: true; mode: "function" | "difference"; params: string[]; usesX: boolean; evaluate: Evaluate }
  | { ok: false; reason: string };

type Token = { t: "num" | "id" | "cmd" | "sym"; v: string };
type F = Evaluate;

class Refusal extends Error {}
const fail = (reason: string): never => { throw new Refusal(reason); };

const spacing = new Set(["quad", "qquad", "enspace", "thinspace", "medspace", "thickspace", "negthinspace", "displaystyle", "textstyle", "limits"]);
const sizing = new Set(["left", "right", "mleft", "mright", "big", "Big", "bigg", "Bigg", "bigl", "bigr", "Bigl", "Bigr", "biggl", "biggr"]);
const aliases: Record<string, Token> = {
  cdot: { t: "sym", v: "*" }, times: { t: "sym", v: "*" }, ast: { t: "sym", v: "*" }, div: { t: "sym", v: "/" },
  lvert: { t: "sym", v: "|" }, rvert: { t: "sym", v: "|" }, vert: { t: "sym", v: "|" },
  lparen: { t: "sym", v: "(" }, rparen: { t: "sym", v: ")" }, lbrack: { t: "sym", v: "[" }, rbrack: { t: "sym", v: "]" },
  pi: { t: "id", v: "π" }, exponentialE: { t: "id", v: "e" },
  dfrac: { t: "cmd", v: "frac" }, tfrac: { t: "cmd", v: "frac" }, cfrac: { t: "cmd", v: "frac" },
};
const sec = (v: number) => 1 / Math.cos(v);
const csc = (v: number) => 1 / Math.sin(v);
const cot = (v: number) => 1 / Math.tan(v);
const functions: Record<string, (v: number) => number> = {
  sin: Math.sin, cos: Math.cos, tan: Math.tan, sec, csc, cot,
  arcsin: Math.asin, arccos: Math.acos, arctan: Math.atan,
  sinh: Math.sinh, cosh: Math.cosh, tanh: Math.tanh,
  ln: Math.log, log: Math.log10, exp: Math.exp,
};
const inverses: Record<string, string> = { sin: "arcsin", cos: "arccos", tan: "arctan" };
const named: Record<string, string> = { "(": ")", "[": "]", "{": "}" };
const relations = new Set(["le", "leq", "ge", "geq", "lt", "gt", "ne", "neq", "approx", "equiv", "sim", "leqslant", "geqslant"]);

function tokenize(latex: string): Token[] {
  const out: Token[] = [];
  let i = 0;
  while (i < latex.length) {
    const c = latex[i]!;
    if (/\s|~|&/.test(c)) { i++; continue; }
    if (c === "\\") {
      const rest = latex.slice(i + 1);
      const word = /^[a-zA-Z]+/.exec(rest)?.[0];
      if (!word) {
        const next = rest[0] ?? "";
        i += 2;
        if (",;:! ".includes(next)) continue;
        if (next === "{") { out.push({ t: "sym", v: "(" }); continue; }
        if (next === "}") { out.push({ t: "sym", v: ")" }); continue; }
        fail(`\\${next} isn't in the plottable set.`);
      }
      i += 1 + word!.length;
      if (spacing.has(word!)) continue;
      if (sizing.has(word!)) {
        // \left. and \right. are invisible fences.
        while (latex[i] === " ") i++;
        if (latex[i] === ".") i++;
        continue;
      }
      out.push(aliases[word!] ?? { t: "cmd", v: word! });
      continue;
    }
    if (/[0-9]/.test(c) || (c === "." && /[0-9]/.test(latex[i + 1] ?? ""))) {
      const number = /^[0-9]*\.?[0-9]*/.exec(latex.slice(i))![0];
      out.push({ t: "num", v: number });
      i += number.length;
      continue;
    }
    if (/[a-zA-Z]/.test(c)) { out.push({ t: "id", v: c }); i++; continue; }
    if (c === "·" || c === "×") { out.push({ t: "sym", v: "*" }); i++; continue; }
    if (c === "−") { out.push({ t: "sym", v: "-" }); i++; continue; }
    if (c === "π") { out.push({ t: "id", v: "π" }); i++; continue; }
    if ("+-*/^_()[]{}|=".includes(c)) { out.push({ t: "sym", v: c }); i++; continue; }
    if ("<>".includes(c)) fail("Inequalities can't be plotted.");
    fail(`"${c}" isn't in the plottable set.`);
  }
  return out;
}

class Parser {
  i = 0;
  abs = 0;
  params: string[] = [];
  usesX = false;
  constructor(readonly tokens: Token[]) {}

  peek(offset = 0) { return this.tokens[this.i + offset]; }
  is(v: string, offset = 0) { const tok = this.peek(offset); return tok?.t === "sym" && tok.v === v; }
  take() { return this.tokens[this.i++]; }
  expect(v: string) { if (!this.is(v)) fail(this.peek() ? `Expected "${v}".` : `Missing "${v}".`); this.i++; }

  expression(): F {
    let left = this.term();
    while (this.is("+") || this.is("-")) {
      const op = this.take()!.v, a = left, b = this.term();
      left = op === "+" ? (x, p) => a(x, p) + b(x, p) : (x, p) => a(x, p) - b(x, p);
    }
    return left;
  }

  startsFactor(tok = this.peek()) {
    if (!tok) return false;
    if (tok.t !== "sym") return true;
    return tok.v === "(" || tok.v === "[" || tok.v === "{" || (tok.v === "|" && this.abs === 0);
  }

  term(): F {
    let left = this.unary();
    for (;;) {
      if (this.is("*") || this.is("/")) {
        const op = this.take()!.v, a = left, b = this.unary();
        left = op === "*" ? (x, p) => a(x, p) * b(x, p) : (x, p) => a(x, p) / b(x, p);
      } else if (this.startsFactor()) {
        const a = left, b = this.power();
        left = (x, p) => a(x, p) * b(x, p);
      } else return left;
    }
  }

  unary(): F {
    if (this.is("-")) { this.i++; const a = this.unary(); return (x, p) => -a(x, p); }
    if (this.is("+")) { this.i++; return this.unary(); }
    return this.power();
  }

  power(): F {
    const base = this.atom();
    if (this.is("_")) fail("Subscripts can't be plotted.");
    if (!this.is("^")) return base;
    this.i++;
    const exponent = this.argument();
    return (x, p) => Math.pow(base(x, p), exponent(x, p));
  }

  /** A braced group, or one token: `x^2`, `\frac12` (a multi-digit number gives its first digit). */
  argument(): F {
    if (this.is("{")) { this.i++; const inner = this.expression(); this.expect("}"); return inner; }
    if (this.is("-")) { this.i++; const a = this.argument(); return (x, p) => -a(x, p); }
    const tok = this.peek();
    if (tok?.t === "num" && tok.v.length > 1) {
      this.tokens.splice(this.i, 1, { t: "num", v: tok.v[0]! }, { t: "num", v: tok.v.slice(1) });
    }
    return this.atom();
  }

  /** `\sin x`, `\sin 2x`, `\sin(x+1)`: a run of factors up to an operator or the next function. */
  functionArgument(): F {
    if (this.is("(") || this.is("[")) return this.power();
    if (!this.startsFactor()) fail("A function is missing its argument.");
    let value = this.power();
    while (this.startsFactor() && !(this.peek()?.t === "cmd" && this.peek()!.v in functions)) {
      const a = value, b = this.power();
      value = (x, p) => a(x, p) * b(x, p);
    }
    return value;
  }

  group(close: string): F {
    const saved = this.abs;
    this.abs = 0;
    const inner = this.expression();
    this.abs = saved;
    this.expect(close);
    return inner;
  }

  atom(): F {
    const tok = this.take();
    if (!tok) fail("The line ends too early.");
    if (tok!.t === "num") { const value = Number(tok!.v); if (!Number.isFinite(value)) fail("A number is malformed."); return () => value; }
    if (tok!.t === "id") return this.letter(tok!.v);
    if (tok!.t === "sym") {
      const close = named[tok!.v];
      if (close) return this.group(close);
      if (tok!.v === "|") {
        this.abs++;
        const inner = this.expression();
        this.abs--;
        this.expect("|");
        return (x, p) => Math.abs(inner(x, p));
      }
      if (tok!.v === "=") fail("Only one = sign can be plotted.");
      fail(`"${tok!.v}" is out of place.`);
    }
    return this.command(tok!.v);
  }

  letter(v: string): F {
    if (v === "x") { this.usesX = true; return (x) => x; }
    if (v === "e") return () => Math.E;
    if (v === "π") return () => Math.PI;
    if (v === "y") fail("y has to stand alone on one side of the =.");
    if (!this.params.includes(v)) this.params.push(v);
    return (_x, p) => p[v] ?? 1;
  }

  command(name: string): F {
    if (name === "frac") {
      const top = this.argument(), bottom = this.argument();
      return (x, p) => top(x, p) / bottom(x, p);
    }
    if (name === "sqrt") {
      let index: F | undefined;
      if (this.is("[")) { this.i++; index = this.group("]"); }
      const radicand = this.argument();
      if (!index) return (x, p) => Math.sqrt(radicand(x, p));
      const n = index;
      return (x, p) => {
        const v = radicand(x, p), k = n(x, p);
        // Odd roots of negatives are real: ∛−8 = −2.
        return v < 0 && Number.isInteger(k) && Math.abs(k) % 2 === 1 ? -Math.pow(-v, 1 / k) : Math.pow(v, 1 / k);
      };
    }
    if (name === "mathrm" || name === "operatorname" || name === "mathit" || name === "mathnormal") {
      this.expect("{");
      let word = "";
      while (this.peek()?.t === "id") word += this.take()!.v;
      this.expect("}");
      if (word in functions) return this.applyFunction(word);
      if (word.length === 1) return this.letter(word);
      fail(`"${word}" isn't in the plottable set.`);
    }
    if (name in functions) return this.applyFunction(name);
    if (name === "placeholder") fail("Fill in the empty box first.");
    if (name === "text" || name === "textrm" || name === "mbox") fail("Words can't be plotted.");
    if (name === "pm" || name === "mp") fail("± gives two values; write each on its own line to plot it.");
    return fail(`\\${name} isn't in the plottable set.`);
  }

  applyFunction(name: string): F {
    let fn = functions[name]!;
    let exponent: F | undefined;
    if (name === "log" && this.is("_")) {
      this.i++;
      const base = this.argument();
      const argument = this.functionArgument();
      return (x, p) => Math.log(argument(x, p)) / Math.log(base(x, p));
    }
    if (this.is("^")) {
      const start = this.i + 1;
      this.i++;
      const power = this.argument();
      const text = this.tokens.slice(start, this.i).map(tok => tok.v).join("");
      if ((text === "{-1}" || text === "-1") && inverses[name]) fn = functions[inverses[name]!]!;
      else exponent = power;
    }
    const argument = this.functionArgument();
    const f = fn;
    if (!exponent) return (x, p) => f(argument(x, p));
    const power = exponent;
    return (x, p) => Math.pow(f(argument(x, p)), power(x, p));
  }
}

function topLevelEquals(tokens: Token[]) {
  const at: number[] = [];
  let depth = 0;
  tokens.forEach((tok, index) => {
    if (tok.t !== "sym") return;
    if ("([{".includes(tok.v)) depth++;
    else if (")]}".includes(tok.v)) depth--;
    else if (tok.v === "=" && depth === 0) at.push(index);
  });
  return at;
}
const isY = (side: Token[]) => side.length === 1 && side[0]!.t === "id" && side[0]!.v === "y";
const isFunctionName = (side: Token[]) =>
  side.length === 4 && side[0]!.t === "id" && side[0]!.v !== "x" && side[1]!.v === "(" && side[2]!.v === "x" && side[3]!.v === ")";

function parseSide(tokens: Token[], into: { params: string[]; usesX: boolean }): F {
  if (!tokens.length) fail("One side of the = is empty.");
  const parser = new Parser(tokens);
  const value = parser.expression();
  if (parser.i < tokens.length) fail(`"${tokens[parser.i]!.v}" is out of place.`);
  for (const name of parser.params) if (!into.params.includes(name)) into.params.push(name);
  into.usesX ||= parser.usesX;
  return value;
}

/** Compiles one line of LaTeX into a plottable function, or says why it can't. */
export function compilePlot(latex: string): Plot {
  try {
    const tokens = tokenize(latex);
    if (!tokens.length) return { ok: false, reason: "The line is empty." };
    if (tokens.some(tok => tok.t === "cmd" && relations.has(tok.v))) fail("Only an = can be plotted, not ≤, ≈ or ≠.");
    const equals = topLevelEquals(tokens);
    if (equals.length > 1) fail("Only one = sign can be plotted.");
    const found = { params: [] as string[], usesX: false };
    const finish = (mode: "function" | "difference", f: F): Plot => {
      const evaluate: Evaluate = (x, p) => { const y = f(x, p); return Number.isFinite(y) ? y : NaN; };
      return { ok: true, mode, params: found.params, usesX: found.usesX, evaluate };
    };
    if (!equals.length) return finish("function", parseSide(tokens, found));
    const left = tokens.slice(0, equals[0]), right = tokens.slice(equals[0]! + 1);
    if (isY(left) || isFunctionName(left)) return finish("function", parseSide(right, found));
    if (isY(right)) return finish("function", parseSide(left, found));
    const a = parseSide(left, found), b = parseSide(right, found);
    if (!found.usesX) fail("There is no x to vary.");
    return finish("difference", (x, p) => a(x, p) - b(x, p));
  } catch (error) {
    if (error instanceof Refusal) return { ok: false, reason: error.message };
    if (error instanceof RangeError) return { ok: false, reason: "The line is nested too deeply." };
    throw error;
  }
}

/** Names a parameter may take: one Latin letter that is not x, y or e. */
export const isParamName = (name: string) => /^[a-zA-Z]$/.test(name) && !["x", "y", "e"].includes(name);
