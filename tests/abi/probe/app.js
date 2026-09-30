// Native platform probe over the conformance schema. Not sealed: Swift tests swap it
// into disposable copies of tests/fixtures/checklist to drive host behavior (drafts,
// capture, toolbar, save ordering). Sealed ABI behavior lives in the consumer fixtures.
export default {
  mount(ctx, target) {
    const doc = ctx.document;
    const root = document.createElement("main");
    root.dataset.hitslopRoot = "";
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
    // A dedicated export surface, prepared only during capture.
    globalThis.selectedView = "Default view";
    const surface = document.createElement("section");
    surface.hidden = true;
    surface.style.cssText = "width:100vw;min-height:300px;background:white";
    const unregister = ctx.capture.registerTarget("export", {
      element: surface,
      prepare() {
        surface.hidden = false;
        const title = document.createElement("h1");
        title.textContent = doc.current.title;
        const selection = document.createElement("p");
        selection.textContent = globalThis.selectedView;
        surface.replaceChildren(title, selection);
      },
      restore() {
        surface.hidden = true;
        surface.replaceChildren();
      },
    });
    surface.dataset.slopCaptureTarget = "";
    target.append(root, surface);
    const binding = ctx.bind.text(input, doc.fields.title);
    return {
      unmount() {
        binding.destroy();
        unregister();
        stop();
        target.replaceChildren();
      },
    };
  },
};
