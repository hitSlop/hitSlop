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
    // Scalar handles expose `value`: assigning shows it at once and commits once it settles.
    const control = (element, handle, read) => {
      const commit = () => { handle.value = read(element.value); };
      element.addEventListener("change", commit);
      return { sync: () => { element.value = String(handle.value); }, destroy: () => element.removeEventListener("change", commit) };
    };
    const controls = [control(ratio, doc.fields.ratio, Number), control(currency, doc.fields.currency, String)];
    const stop = doc.subscribe(() => { render(); controls.forEach((c) => c.sync()); });
    render();
    controls.forEach((c) => c.sync());
    return {
      unmount() {
        stop();
        controls.forEach((c) => c.destroy());
        target.replaceChildren();
      },
    };
  },
};
