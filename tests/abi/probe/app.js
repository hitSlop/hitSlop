// Native platform probe over the conformance schema. Not sealed: Swift tests swap it
// into disposable copies of tests/fixtures/checklist to drive host behavior (drafts,
// live export, toolbar, save ordering). Sealed ABI behavior lives in the consumer fixtures.
export default {
  mount(ctx, target) {
    const doc = ctx.document;
    const root = document.createElement("main");
    root.dataset.tinyRoot = "";
    root.style.cssText = "font:18px sans-serif;padding:12px";
    const input = document.createElement("input");
    input.id = "draft";
    input.setAttribute("aria-label", "Draft");
    const edit = document.createElement("button");
    edit.id = "edit";
    edit.textContent = "Edit";
    // One authored scenario: text, scalars, row move and a row edit in one change.
    edit.onclick = () =>
      doc.change((tx) => {
        const rows = doc.current.rows;
        tx.fields.title.set("Edited 🦊 café");
        tx.fields.hits.increment(1);
        tx.fields.rows.move(rows[0].$id, { after: rows.at(-1).$id });
        tx.fields.rows.item(rows[0].$id).text.set("changed");
      });
    const heading = document.createElement("h1");
    const render = () => (heading.textContent = doc.current.title);
    const stop = doc.subscribe(render);
    render();
    root.append(heading, input, edit);
    target.append(root);
    const binding = ctx.bind.text(input, doc.fields.title);
    return {
      unmount() {
        binding.destroy();
        stop();
        target.replaceChildren();
      },
    };
  },
};
