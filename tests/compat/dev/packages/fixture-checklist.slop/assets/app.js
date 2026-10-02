// Plain-JS consumer of the ctx interface. No engine or private bridge imports.
export default {
  mount(ctx, target) {
    const doc = ctx.document;
    const output = document.createElement("p");
    const input = document.createElement("textarea");
    target.append(output, input);
    const render = () => { output.textContent = doc.current.title; };
    const stop = doc.subscribe(render);
    const binding = ctx.bind.text(input, doc.fields.title);
    render();
    globalThis.contractTest = async () => {
      const before = doc.current.hits;
      await doc.fields.hits.increment(2);
      if (doc.current.hits !== before + 2) throw new Error("Promise resolved before publication");
      const id = await doc.change(tx => {
        const { id } = tx.fields.rows.insert({ text: "ABI 2", done: false });
        tx.fields.rows.item(id).done.set(true);
        return id;
      });
      if (!doc.current.rows.find(row => row.$id === id)?.done) throw new Error("Collector lost inserted row edit");
      await doc.fields.rows.remove(id);
      await doc.fields.hits.increment(-2);
      // A resolved flush means every accepted edit is saved.
      await doc.flush();
      return true;
    };
    return { unmount() { stop(); binding.destroy(); output.remove(); input.remove(); } };
  },
};
