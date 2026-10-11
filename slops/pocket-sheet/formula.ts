// A tiny spreadsheet language: numbers, text, cell references, ranges, arithmetic,
// comparisons, `&` joins and a handful of Excel-compatible functions.

export const COLUMNS = 6;
export const ROWS = 12;
export const columnName = (column: number) => String.fromCharCode(65 + column);
export const cellKey = (column: number, row: number) => `${columnName(column)}${row + 1}`;
export const allKeys = Array.from({ length: ROWS }, (_, row) =>
  Array.from({ length: COLUMNS }, (_, column) => cellKey(column, row))).flat();

export function parseKey(key: string): { column: number; row: number } | undefined {
  const match = /^([A-Z])(\d{1,2})$/.exec(key.toUpperCase());
  if (!match) return;
  const column = match[1]!.charCodeAt(0) - 65, row = Number(match[2]) - 1;
  return column < COLUMNS && row >= 0 && row < ROWS ? { column, row } : undefined;
}

export type ErrorCode = "#REF!" | "#DIV/0!" | "#VALUE!" | "#NAME?" | "#ERR!" | "#CYCLE!";
export type Value = number | string | boolean | { error: ErrorCode };
export const isError = (value: Value): value is { error: ErrorCode } => typeof value === "object";

type Token =
  | { kind: "number"; value: number }
  | { kind: "string"; value: string }
  | { kind: "ref"; value: string }
  | { kind: "name"; value: string }
  | { kind: "op"; value: string };
type Expr =
  | { kind: "literal"; value: Value }
  | { kind: "ref"; key: string }
  | { kind: "range"; from: string; to: string }
  | { kind: "unary"; op: string; arg: Expr }
  | { kind: "binary"; op: string; left: Expr; right: Expr }
  | { kind: "call"; name: string; args: Expr[] };

class FormulaError extends Error {
  constructor(readonly code: ErrorCode) { super(code); }
}

function tokenize(source: string): Token[] {
  const tokens: Token[] = [];
  let i = 0;
  while (i < source.length) {
    const rest = source.slice(i);
    let match: RegExpExecArray | null;
    if (/^\s/.test(rest)) { i++; continue; }
    if ((match = /^(\d+(\.\d*)?|\.\d+)(%?)/.exec(rest))) {
      tokens.push({ kind: "number", value: Number(match[1]) / (match[3] ? 100 : 1) });
    } else if ((match = /^"((?:[^"]|"")*)"/.exec(rest))) {
      tokens.push({ kind: "string", value: match[1]!.replaceAll('""', '"') });
    } else if ((match = /^\$?([A-Za-z])\$?(\d+)(?![\w(])/.exec(rest))) {
      tokens.push({ kind: "ref", value: `${match[1]!.toUpperCase()}${match[2]}` });
    } else if ((match = /^[A-Za-z][A-Za-z0-9.]*/.exec(rest))) {
      tokens.push({ kind: "name", value: match[0].toUpperCase() });
    } else if ((match = /^(<=|>=|<>|[-+*/^&=<>(),:])/.exec(rest))) {
      tokens.push({ kind: "op", value: match[0] });
    } else throw new FormulaError("#ERR!");
    i += match[0].length;
  }
  return tokens;
}

function parse(source: string): Expr {
  const tokens = tokenize(source);
  let at = 0;
  const peek = () => tokens[at];
  const isOp = (...ops: string[]) => { const t = peek(); return t?.kind === "op" && ops.includes(t.value); };
  const take = (op: string) => { if (!isOp(op)) throw new FormulaError("#ERR!"); at++; };
  const binary = (next: () => Expr, ops: string[]) => () => {
    let left = next();
    while (isOp(...ops)) { const op = (tokens[at++] as { value: string }).value; left = { kind: "binary", op, left, right: next() }; }
    return left;
  };
  const primary = (): Expr => {
    const token = tokens[at++];
    if (!token) throw new FormulaError("#ERR!");
    if (token.kind === "number" || token.kind === "string") return { kind: "literal", value: token.value };
    if (token.kind === "ref") {
      if (isOp(":")) {
        at++;
        const end = tokens[at++];
        if (end?.kind !== "ref") throw new FormulaError("#ERR!");
        return { kind: "range", from: token.value, to: end.value };
      }
      return { kind: "ref", key: token.value };
    }
    if (token.kind === "name") {
      if (token.value === "TRUE" || token.value === "FALSE") return { kind: "literal", value: token.value === "TRUE" };
      take("(");
      const args: Expr[] = [];
      if (!isOp(")")) do args.push(comparison()); while (isOp(",") && ++at);
      take(")");
      return { kind: "call", name: token.value, args };
    }
    if (token.value === "(") { const inner = comparison(); take(")"); return inner; }
    if (token.value === "-" || token.value === "+") return { kind: "unary", op: token.value, arg: power() };
    throw new FormulaError("#ERR!");
  };
  const power: () => Expr = binary(primary, ["^"]);
  const product = binary(power, ["*", "/"]);
  const sum = binary(product, ["+", "-"]);
  const join = binary(sum, ["&"]);
  const comparison = binary(join, ["=", "<>", "<", ">", "<=", ">="]);
  const expr = comparison();
  if (at !== tokens.length) throw new FormulaError("#ERR!");
  return expr;
}

/** Plain entries become numbers when they look like one ("24", "4.5", "15%", "-3"). */
export function literal(input: string): Value {
  const trimmed = input.trim();
  const match = /^(-?)(\d+(?:\.\d*)?|\.\d+)(%?)$/.exec(trimmed.replaceAll(",", ""));
  if (match) return Number(match[1] + match[2]) / (match[3] ? 100 : 1);
  return trimmed;
}

type Evaluated = Map<string, Value>;

export function evaluate(inputs: Readonly<Record<string, string>>): Evaluated {
  const results: Evaluated = new Map();
  const visiting = new Set<string>();
  const cell = (key: string): Value => {
    if (!parseKey(key)) throw new FormulaError("#REF!");
    const known = results.get(key);
    if (known !== undefined) return known;
    if (visiting.has(key)) throw new FormulaError("#CYCLE!");
    visiting.add(key);
    let value: Value;
    const input = inputs[key] ?? "";
    try {
      value = input.startsWith("=") ? run(parse(input.slice(1))) : literal(input);
    } catch (error) {
      if (!(error instanceof FormulaError)) throw error;
      // Every cell that feeds a cycle reports it; downstream cells see the cycle as a value.
      value = { error: error.code };
    }
    visiting.delete(key);
    results.set(key, value);
    return value;
  };
  const range = (from: string, to: string): Value[] => {
    const a = parseKey(from), b = parseKey(to);
    if (!a || !b) throw new FormulaError("#REF!");
    const values: Value[] = [];
    for (let row = Math.min(a.row, b.row); row <= Math.max(a.row, b.row); row++)
      for (let column = Math.min(a.column, b.column); column <= Math.max(a.column, b.column); column++)
        values.push(cell(cellKey(column, row)));
    return values;
  };
  const number = (value: Value): number => {
    if (isError(value)) throw new FormulaError(value.error);
    if (typeof value === "number") return value;
    if (typeof value === "boolean") return value ? 1 : 0;
    if (value === "") return 0;
    const parsed = literal(value);
    if (typeof parsed === "number") return parsed;
    throw new FormulaError("#VALUE!");
  };
  const text = (value: Value): string => {
    if (isError(value)) throw new FormulaError(value.error);
    if (typeof value === "boolean") return value ? "TRUE" : "FALSE";
    return typeof value === "number" ? format(value) : value;
  };
  // Functions see ranges flattened; text and blanks inside ranges are skipped like Excel.
  const numbers = (args: Expr[]) => args.flatMap(arg => {
    if (arg.kind !== "range") return [number(run(arg))];
    return range(arg.from, arg.to).flatMap(value => {
      if (isError(value)) throw new FormulaError(value.error);
      return typeof value === "number" ? [value] : [];
    });
  });
  const functions: Record<string, (args: Expr[]) => Value> = {
    SUM: args => numbers(args).reduce((a, b) => a + b, 0),
    AVERAGE: args => { const n = numbers(args); if (!n.length) throw new FormulaError("#DIV/0!"); return n.reduce((a, b) => a + b, 0) / n.length; },
    MIN: args => { const n = numbers(args); return n.length ? Math.min(...n) : 0; },
    MAX: args => { const n = numbers(args); return n.length ? Math.max(...n) : 0; },
    COUNT: args => numbers(args).length,
    ABS: args => { arity(args, 1); return Math.abs(number(run(args[0]!))); },
    ROUND: args => {
      arity(args, 1, 2);
      const digits = args[1] ? Math.trunc(number(run(args[1]))) : 0, scale = 10 ** digits;
      return Math.round(number(run(args[0]!)) * scale) / scale;
    },
    IF: args => {
      arity(args, 2, 3);
      return truthy(run(args[0]!)) ? run(args[1]!) : args[2] ? run(args[2]) : false;
    },
  };
  const truthy = (value: Value) => typeof value === "string" ? value !== "" : Boolean(number(value));
  const run = (expr: Expr): Value => {
    switch (expr.kind) {
      case "literal": return expr.value;
      case "ref": {
        const value = cell(expr.key);
        if (isError(value)) throw new FormulaError(value.error);
        return value;
      }
      case "range": throw new FormulaError("#VALUE!");
      case "unary": return expr.op === "-" ? -number(run(expr.arg)) : number(run(expr.arg));
      case "call": {
        const fn = functions[expr.name];
        if (!fn) throw new FormulaError("#NAME?");
        return fn(expr.args);
      }
      case "binary": {
        const left = run(expr.left), right = run(expr.right);
        switch (expr.op) {
          case "&": return text(left) + text(right);
          case "+": return number(left) + number(right);
          case "-": return number(left) - number(right);
          case "*": return number(left) * number(right);
          case "/": { const d = number(right); if (d === 0) throw new FormulaError("#DIV/0!"); return number(left) / d; }
          case "^": { const r = number(left) ** number(right); if (!Number.isFinite(r)) throw new FormulaError("#VALUE!"); return r; }
          default: return compare(expr.op, left, right);
        }
      }
    }
  };
  const compare = (op: string, left: Value, right: Value): boolean => {
    const bothText = typeof left === "string" && typeof right === "string";
    const a = bothText ? (left as string).toLowerCase() : number(left);
    const b = bothText ? (right as string).toLowerCase() : number(right);
    switch (op) {
      case "=": return a === b;
      case "<>": return a !== b;
      case "<": return a < b;
      case ">": return a > b;
      case "<=": return a <= b;
      default: return a >= b;
    }
  };
  for (const key of allKeys) cell(key);
  return results;
}

function arity(args: Expr[], min: number, max = min) {
  if (args.length < min || args.length > max) throw new FormulaError("#VALUE!");
}

/** Up to two decimals, grouped, and never scientific notation for everyday sizes. */
export function format(value: number): string {
  if (!Number.isFinite(value)) return "#VALUE!";
  return new Intl.NumberFormat("en-US", { maximumFractionDigits: Math.abs(value) < 1 && value !== 0 ? 4 : 2 }).format(value);
}

export function display(value: Value): string {
  if (isError(value)) return value.error;
  if (typeof value === "boolean") return value ? "TRUE" : "FALSE";
  return typeof value === "number" ? format(value) : value;
}

/** Excel-style CSV: raw entries, so formulas travel as `=…` and recompute when opened.
 *  Trimmed to the used area; a BOM keeps Excel from misreading UTF-8 such as emoji. */
export function csv(inputs: Readonly<Record<string, string>>): string {
  let lastRow = -1, lastColumn = -1;
  for (const [key, input] of Object.entries(inputs)) {
    const point = parseKey(key);
    if (!point || !input) continue;
    lastRow = Math.max(lastRow, point.row);
    lastColumn = Math.max(lastColumn, point.column);
  }
  const field = (value: string) => /[",\r\n]|^\s|\s$/.test(value) ? `"${value.replaceAll('"', '""')}"` : value;
  const lines = Array.from({ length: lastRow + 1 }, (_, row) =>
    Array.from({ length: lastColumn + 1 }, (_, column) => field(inputs[cellKey(column, row)] ?? "")).join(","));
  return "\uFEFF" + lines.map(line => line + "\r\n").join("");
}
