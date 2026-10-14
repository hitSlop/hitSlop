<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import { getDocument } from "hitslop/svelte";
  import Plus from "@lucide/svelte/icons/plus";
  import X from "@lucide/svelte/icons/x";
  import schema from "./schema";
  import Chart from "./Chart.svelte";
  import { addRow, kinds, notebookView, parseNumber, removeRow, setAxis, setChart, setTitle, updateRow, type Kind } from "./lab";

  const doc = getDocument(schema);
  const data = $derived(doc.current);
  const view = $derived(notebookView(data));
  let stageWidth = $state(640);
  const stageHeight = $derived(Math.round(Math.min(420, Math.max(250, stageWidth * 0.58))));
  const kindName: Record<Kind, string> = { scatter: "Scatter", line: "Line", bar: "Bar" };

  // Number cells: a value that is not a number puts the old one back instead of storing it.
  function number(event: Event, id: string, field: "x" | "y" | "err", old: number) {
    const input = event.currentTarget as HTMLInputElement;
    const value = parseNumber(input.value);
    if (value === null || (field === "err" && value < 0)) { input.value = String(old); return; }
    doc.change(tx => updateRow(tx, id, { [field]: value }));
  }
  function add() { doc.change(tx => addRow(tx, doc.current)); }
</script>

<main class="lab" aria-label="Lab notebook">
  <header class="lab-head">
    <input class="lab-title" aria-label="Figure title" placeholder="Name this figure" maxlength="60" value={data.title}
      onchange={event => doc.change(tx => setTitle(tx, event.currentTarget.value))} />
    <div class="lab-kinds" role="radiogroup" aria-label="Chart kind">
      {#each kinds as kind}
        <label data-on={data.kind === kind}>
          <input type="radio" name="kind" value={kind} checked={data.kind === kind}
            onchange={() => doc.change(tx => setChart(tx, { kind }))} />
          {kindName[kind]}
        </label>
      {/each}
    </div>
  </header>

  <section class="lab-figure" aria-label="Figure" bind:clientWidth={stageWidth}>
    <Chart {data} width={Math.max(0, stageWidth - 2)} height={stageHeight} />
  </section>

  <section class="lab-fit" aria-label="Fit" data-on={!!view.fit}>
    <div class="lab-switches">
      <label class="lab-switch"><input type="checkbox" checked={data.fit} onchange={event => doc.change(tx => setChart(tx, { fit: event.currentTarget.checked }))} /><span>Least squares line</span></label>
      <label class="lab-switch"><input type="checkbox" checked={data.errorBars} onchange={event => doc.change(tx => setChart(tx, { errorBars: event.currentTarget.checked }))} /><span>Error bars</span></label>
    </div>
    {#if view.readout}
      <dl class="lab-numbers">
        <div><dt>Slope</dt><dd>{view.readout.slope}</dd></div>
        <div><dt>Intercept</dt><dd>{view.readout.intercept}</dd></div>
        <div><dt>R&sup2;</dt><dd>{view.readout.r2}</dd></div>
        <div><dt>Rows</dt><dd>{view.readout.n}</dd></div>
      </dl>
      <p class="lab-honest">Ordinary least squares: a description of these rows, not a test. No significance is claimed.</p>
    {:else if view.note}
      <p class="lab-note" role="status">{view.note}</p>
    {:else}
      <p class="lab-honest">Switch on the line to fit these rows by ordinary least squares (slope, intercept, R&sup2;).</p>
    {/if}
  </section>

  <section class="lab-axes" aria-label="Axes">
    {#each [["x", "Horizontal", data.xLabel, data.xUnit], ["y", "Vertical", data.yLabel, data.yUnit]] as const as [axis, name, label, unit]}
      <fieldset>
        <legend>{name} axis</legend>
        <input aria-label={`${name} axis label`} placeholder="Quantity" maxlength="32" value={label}
          onchange={event => doc.change(tx => setAxis(tx, axis, { label: event.currentTarget.value }))} />
        <input class="unit" aria-label={`${name} axis unit`} placeholder="unit" maxlength="12" value={unit}
          onchange={event => doc.change(tx => setAxis(tx, axis, { unit: event.currentTarget.value }))} />
      </fieldset>
    {/each}
  </section>

  <section class="lab-table" aria-label="Measurements">
    <table>
      <thead>
        <tr>
          <th scope="col">{data.kind === "bar" ? "Category" : "#"}</th>
          <th scope="col">x{data.xUnit ? ` (${data.xUnit})` : ""}</th>
          <th scope="col">y{data.yUnit ? ` (${data.yUnit})` : ""}</th>
          <th scope="col">&plusmn; y</th>
          <th scope="col"><span class="sr">Remove</span></th>
        </tr>
      </thead>
      <tbody>
        {#each data.rows as row, i (row.$id)}
          <tr>
            <td>
              {#if data.kind === "bar"}
                <input aria-label={`Row ${i + 1} category`} placeholder={String(row.x)} maxlength="24" value={row.label}
                  onchange={event => doc.change(tx => updateRow(tx, row.$id, { label: event.currentTarget.value }))} />
              {:else}<span class="idx">{i + 1}</span>{/if}
            </td>
            <td><input inputmode="decimal" aria-label={`Row ${i + 1} x`} value={row.x} onchange={event => number(event, row.$id, "x", row.x)} /></td>
            <td><input inputmode="decimal" aria-label={`Row ${i + 1} y`} value={row.y} onchange={event => number(event, row.$id, "y", row.y)} /></td>
            <td><input inputmode="decimal" aria-label={`Row ${i + 1} uncertainty`} value={row.err} onchange={event => number(event, row.$id, "err", row.err)} /></td>
            <td><button class="icon" aria-label={`Remove row ${i + 1}`} onclick={() => doc.change(tx => removeRow(tx, row.$id))}><X size={16} /></button></td>
          </tr>
        {/each}
      </tbody>
    </table>
    <button class="add" onclick={add} disabled={!view.canAdd}><Plus size={16} /> Add a measurement</button>
  </section>
</main>
