// Plain-JS consumer of the ctx interface for records, scalar lists and optional text.
export default {
  mount(ctx, target) {
    const doc = ctx.document;
    const output = document.createElement("p");
    const notes = document.createElement("textarea");
    target.append(output, notes);
    const render = () => {
      const value = doc.current;
      output.textContent = `${value.title}: ${Object.keys(value.done).length} done, ${value.pixels.join(" ")}, presets ${value.presets.join("/")}`;
    };
    const stop = doc.subscribe(render);
    const binding = ctx.bind.text(notes, doc.fields.notes);
    render();
    return {
      unmount() {
        stop();
        binding.destroy();
        target.replaceChildren();
      },
    };
  },
};
