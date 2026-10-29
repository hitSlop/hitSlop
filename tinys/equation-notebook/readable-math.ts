// A compact, synchronous notation for saved equations. Unknown LaTeX stays intact
// rather than silently dropping a command or changing its meaning.
export function readableMath(latex: string): string {
  let index = 0;
  const aliases: Record<string, string> = {
    pi: "π", theta: "θ", alpha: "α", beta: "β", gamma: "γ", delta: "δ",
    Delta: "Δ", lambda: "λ", mu: "μ", sigma: "σ", omega: "ω", infinity: "∞", infty: "∞",
    cdot: " · ", times: " × ", div: " ÷ ", pm: " ± ", mp: " ∓ ",
    le: " ≤ ", leq: " ≤ ", ge: " ≥ ", geq: " ≥ ", ne: " ≠ ", neq: " ≠ ",
    approx: " ≈ ", equiv: " ≡ ", to: " → ", rightarrow: " → ",
    quad: "  ", qquad: "    ", left: "", right: "", displaystyle: "",
  };
  const functions = new Set(["sin", "cos", "tan", "sec", "csc", "cot", "arcsin", "arccos", "arctan", "sinh", "cosh", "tanh", "ln", "log", "exp"]);
  function group(): string {
    while (latex[index] === " ") index++;
    if (latex[index++] !== "{") throw new Error("Expected a group");
    return sequence(true);
  }
  function sequence(grouped = false): string {
    let output = "";
    while (index < latex.length) {
      const character = latex[index++]!;
      if (character === "}") {
        if (!grouped) throw new Error("Unmatched group");
        return output;
      }
      if (character === "{") { output += `(${sequence(true)})`; continue; }
      if (character === "\\") {
        const command = /^[a-zA-Z]+/.exec(latex.slice(index))?.[0];
        if (!command) {
          const escaped = latex[index++]!;
          if (",;:! ".includes(escaped)) { output += " "; continue; }
          if ("{}_%$#".includes(escaped)) { output += escaped; continue; }
          throw new Error("Unknown escape");
        }
        index += command.length;
        if (command === "frac" || command === "dfrac" || command === "tfrac") {
          const top = group(), bottom = group();
          output += `(${top}) / (${bottom})`;
        } else if (command === "sqrt") output += `√(${group()})`;
        else if (["text", "mathrm", "mathbf", "mathit", "operatorname"].includes(command)) output += group();
        else if (Object.hasOwn(aliases, command)) {
          output += aliases[command];
          if ((command === "left" || command === "right") && latex[index] === ".") index++;
        }
        else if (functions.has(command)) output += command;
        else throw new Error("Unknown command");
      } else if (character === "^") {
        const power = latex[index] === "{" ? group() : latex[index++];
        if (!power) throw new Error("Missing exponent");
        const superscript: Record<string, string> = {"0":"⁰","1":"¹","2":"²","3":"³","4":"⁴","5":"⁵","6":"⁶","7":"⁷","8":"⁸","9":"⁹","-":"⁻","−":"⁻","+":"⁺"};
        output += [...power].every(value => value in superscript)
          ? [...power].map(value => superscript[value]).join("") : `^(${power})`;
      } else output += character === "-" ? "−" : character === "~" ? " " : character;
    }
    if (grouped) throw new Error("Unclosed group");
    return output;
  }
  try { return sequence(); } catch { return latex; }
}
