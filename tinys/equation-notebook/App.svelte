<script lang="ts">
  import "@hitslop/tinys/base.css";
  import "./mathlive-static.css";
  import "./styles.css";
  import { tick } from "svelte";
  import { EditableText, getDocument, notify } from "@hitslop/tinys/svelte";
  import { DropdownMenu } from "bits-ui";
  import Plus from "@lucide/svelte/icons/plus";
  import Sigma from "@lucide/svelte/icons/sigma";
  import Pilcrow from "@lucide/svelte/icons/pilcrow";
  import ChartSpline from "@lucide/svelte/icons/chart-spline";
  import Ellipsis from "@lucide/svelte/icons/ellipsis";
  import schema, { type Kind } from "./schema";
  import { addCell, moveCell, notebookView, paramValues, plotCaption, setLatex, setParam, setPlot, setWindow, type Cell, type Line } from "./notebook";
  import MathField from "./MathField.svelte";
  import Graph from "./Graph.svelte";
  import Params from "./Params.svelte";

  const doc = getDocument(schema);
  const view = $derived(notebookView(doc.current));
  // View state only: the line being worked on, slider values mid-drag, the open menu.
  let active = $state<string | null>(null);
  let dragging = $state<Record<string, Record<string, number>>>({});
  let menuFor = $state<string | null>(null);
  let menuAnchor = $state<HTMLElement | null>(null);
  const menuIndex = $derived(menuFor ? view.lines.findIndex(line => line.cell.$id === menuFor) : -1);
  $effect(() => { if (menuFor && menuIndex < 0) menuFor = null; });

  const kindLabel: Record<Kind, string> = { equation: "Equation", note: "Note", graph: "Graph" };
  const lineLabel = (line: Line) => line.cell.kind === "note" ? "Note" : `${kindLabel[line.cell.kind]} ${line.number}`;

  async function focusCell(id: string) {
    await tick();
    const target = document.querySelector<HTMLElement>(`[data-cell="${id}"] math-field, [data-cell="${id}"] .note [role="textbox"]`);
    target?.focus();
  }
  async function add(kind: Kind, after?: string) {
    const id = await doc.change(tx => addCell(tx, kind, { after }));
    active = id;
    await focusCell(id);
    document.querySelector(`[data-cell="${id}"]`)?.scrollIntoView({ block: "nearest" });
  }
  /** Enter on a line moves to the next one, or starts a new equation after the last. */
  function next(index: number) {
    const following = view.lines[index + 1];
    if (following) void focusCell(following.cell.$id);
    else void add("equation");
  }
  function commitLatex(cell: Cell, latex: string) {
    if (!doc.current.cells.some(row => row.$id === cell.$id)) return;
    void doc.change(tx => setLatex(tx, doc.current, cell.$id, latex));
  }
  function previewParam(cell: Cell, name: string, value: number) {
    dragging = { ...dragging, [cell.$id]: { ...dragging[cell.$id], [name]: value } };
  }
  async function commitParam(cell: Cell, name: string, value: number) {
    try {
      await doc.change(tx => setParam(tx, doc.current, cell.$id, name, value));
    } finally {
      const { [cell.$id]: _, ...rest } = dragging;
      dragging = rest;
    }
  }
  async function move(id: string, direction: -1 | 1) {
    await doc.change(tx => moveCell(tx, doc.current, id, direction));
  }
  async function remove(id: string) {
    const index = view.lines.findIndex(line => line.cell.$id === id);
    await doc.fields.cells.remove(id);
    notify("Line removed. Undo brings it back.");
    const neighbor = view.lines[Math.min(index, view.lines.length - 1)];
    if (neighbor) void focusCell(neighbor.cell.$id);
  }
  function openMenu(id: string, button: HTMLElement) {
    if (menuFor === id) { menuFor = null; return; }
    menuAnchor = button;
    menuFor = id;
  }
</script>

<main class="pad">
  <header class="title-block">
    <div class="title-cell">
      <span class="block-label">Problem</span>
      <EditableText class="title" field={doc.fields.title} label="Problem" placeholder="Name the problem" />
    </div>
    <div class="block-cell"><span class="block-label">Lines</span><span class="block-value">{view.equations}</span></div>
    <div class="block-cell"><span class="block-label">Sheet</span><span class="block-value">1 of 1</span></div>
  </header>

  <div class="sheet">
    <ol class="lines" aria-label="Worked solution">
      {#each view.lines as line, index (line.cell.$id)}
        {@const cell = line.cell}
        {@const values = line.plot?.ok ? paramValues(cell, line.plot.params, dragging[cell.$id]) : {}}
        <li
          class="line"
          data-kind={cell.kind}
          data-cell={cell.$id}
          data-active={active === cell.$id || undefined}
          onfocusin={() => (active = cell.$id)}
        >
          <div class="line-body" data-graphed={line.graphed || undefined}>
            {#if cell.kind === "note"}
              <EditableText class="note" field={doc.at(cell).note} label="Note" placeholder="Write a note…" onenter={() => next(index)} />
            {:else}
              {#if cell.kind === "graph"}
                <Graph size="large" plot={line.plot} params={values} xMin={cell.xMin} xMax={cell.xMax} label={`Graph ${line.number}`}
                  onwindow={(a, b) => doc.change(tx => setWindow(tx, doc.current, cell.$id, a, b))} />
              {/if}
              <div class="equation">
                <MathField
                  latex={cell.latex}
                  label={lineLabel(line)}
                  oninput={(latex) => doc.at(cell).latex.preview(latex)}
                  oncommit={(latex) => commitLatex(cell, latex)}
                  onenter={() => next(index)}
                />
              </div>
              {#if cell.kind === "equation" && cell.plot}
                <Graph plot={line.plot} params={values} xMin={cell.xMin} xMax={cell.xMax} label={`Graph of equation ${line.number}`}
                  onwindow={(a, b) => doc.change(tx => setWindow(tx, doc.current, cell.$id, a, b))} />
              {/if}
              {#if line.graphed}
                <Params {values} onpreview={(name, value) => previewParam(cell, name, value)} oncommit={(name, value) => commitParam(cell, name, value)} />
                {#if line.plot?.ok && plotCaption(line.plot)}<p class="caption">{plotCaption(line.plot)}</p>{/if}
              {/if}
            {/if}
          </div>
          {#if cell.kind === "equation"}
            <button
              type="button"
              class="tool plot-toggle"
              aria-pressed={cell.plot}
              aria-label={cell.plot ? `Hide the graph of equation ${line.number}` : `Plot equation ${line.number}`}
              title={cell.plot ? "Hide graph" : "Plot this line"}
              onclick={() => doc.change(tx => setPlot(tx, doc.current, cell.$id, !cell.plot))}
            ><ChartSpline size={18} /></button>
          {/if}
          <div class="line-margin">
            {#if line.number}<span class="eq-number" aria-hidden="true">({line.number})</span>{/if}
            <button
              type="button"
              class="tool line-menu"
              aria-label={`Actions for ${lineLabel(line).toLowerCase()}`}
              aria-haspopup="menu"
              aria-expanded={menuFor === cell.$id}
              onclick={(event) => openMenu(cell.$id, event.currentTarget)}
            ><Ellipsis size={17} /></button>
          </div>
        </li>
      {/each}
    </ol>
    {#if !view.lines.length}
      <p class="empty">A clean sheet. Start with an equation below.</p>
    {/if}
  </div>

  <DropdownMenu.Root open={menuFor !== null} onOpenChange={(open) => { if (!open) menuFor = null; }}>
    <DropdownMenu.Portal>
      <DropdownMenu.Content
        class="menu"
        customAnchor={menuAnchor}
        sideOffset={4}
        align="end"
        onInteractOutside={(event) => { if (menuAnchor?.contains(event.target as Node)) event.preventDefault(); }}
        onCloseAutoFocus={(event) => { event.preventDefault(); menuAnchor?.focus(); }}
      >
        {#if menuFor}
          {@const id = menuFor}
          <DropdownMenu.Item onSelect={() => add("equation", id)}>Equation below</DropdownMenu.Item>
          <DropdownMenu.Item onSelect={() => add("note", id)}>Note below</DropdownMenu.Item>
          <DropdownMenu.Item onSelect={() => add("graph", id)}>Graph below</DropdownMenu.Item>
          <DropdownMenu.Separator />
          <DropdownMenu.Item disabled={menuIndex <= 0} onSelect={() => move(id, -1)}>Move up</DropdownMenu.Item>
          <DropdownMenu.Item disabled={menuIndex === view.lines.length - 1} onSelect={() => move(id, 1)}>Move down</DropdownMenu.Item>
          <DropdownMenu.Separator />
          <DropdownMenu.Item onSelect={() => remove(id)}>Remove line</DropdownMenu.Item>
        {/if}
      </DropdownMenu.Content>
    </DropdownMenu.Portal>
  </DropdownMenu.Root>

  <footer class="add-bar" aria-label="Add a line">
    <span class="hint">Enter: next line</span>
    <div class="add-buttons">
      <button type="button" onclick={() => add("equation")}><Plus size={14} /><Sigma size={16} /> Equation</button>
      <button type="button" onclick={() => add("note")}><Plus size={14} /><Pilcrow size={16} /> Note</button>
      <button type="button" onclick={() => add("graph")}><Plus size={14} /><ChartSpline size={16} /> Graph</button>
    </div>
  </footer>
</main>
