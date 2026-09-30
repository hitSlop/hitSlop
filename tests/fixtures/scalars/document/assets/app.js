// Plain-JS consumer of the ctx interface for the scalar kinds. No engine or bridge imports.
export default {
  mount(ctx, target) {
    const doc = ctx.document;
    const output = document.createElement("p");
    const ratio = Object.assign(document.createElement("input"), { type: "range", min: "0", max: "1", step: "0.05" });
    const currency = document.createElement("select");
    for (const value of ["CAD", "USD", "EUR"]) currency.append(new Option(value, value));
    target.append(output, ratio, currency);
    const render = () => {
      const value = doc.current;
      output.textContent = `${value.title}: ${value.currency} ${value.ratio} ${value.memo ?? "(no memo)"}`;
    };
    const stop = doc.subscribe(render);
    const bindings = [ctx.bind.value(ratio, doc.fields.ratio), ctx.bind.value(currency, doc.fields.currency)];
    render();
    return {
      unmount() {
        stop();
        bindings.forEach((binding) => binding.destroy());
        target.replaceChildren();
      },
    };
  },
};
