<script lang="ts">
  import { ui } from "./ui.svelte";
  import { useDocument } from "@hitslop/document/svelte";
  import schema from "./schema";
  import { cellKey, columnName, COLUMNS, display, evaluate, isError, ROWS, type Value } from "./formula";
  const doc = useDocument(schema);
  const inputs = $derived(Object.fromEntries(Object.entries(doc.current.cells).map(([key, cell]) => [key, cell.input])));
  const results = $derived(evaluate(inputs));
  const widthOf = (column: number) => {
    const letter = columnName(column);
    return ui.resizing?.letter === letter ? ui.resizing.width : doc.current.widths[letter] ?? 96;
  };
  const template = $derived(`40px ${Array.from({ length: COLUMNS }, (_, c) => `${widthOf(c)}px`).join(" ")}`);
  const kind = (value: Value | undefined) =>
    value === undefined || value === "" ? "blank" : isError(value) ? "error" : typeof value === "number" ? "number" : "text";
</script>

<article class="pocket-export-view">
  <p class="pocket-eyebrow">Pocket Sheet</p>
  <h1>{doc.current.title || "Untitled sheet"}</h1>
  <div class="pocket-grid" style:grid-template-columns={template}
    role="grid" aria-readonly="true" aria-label={`${doc.current.title || "Pocket Sheet"} cells`}
    aria-rowcount={ROWS + 1} aria-colcount={COLUMNS + 1} tabindex="-1">
    <div class="pocket-row" role="row">
      <span class="pocket-corner" role="columnheader" aria-label="Rows"></span>
      {#each Array.from({ length: COLUMNS }, (_, c) => c) as column}
        <span class="pocket-colhead" role="columnheader">{columnName(column)}</span>
      {/each}
    </div>
    {#each Array.from({ length: ROWS }, (_, r) => r) as row}
      <div class="pocket-row" role="row">
        <span class="pocket-rowhead" role="rowheader">{row + 1}</span>
        {#each Array.from({ length: COLUMNS }, (_, c) => c) as column}
          {@const key = cellKey(column, row)}
          {@const cell = doc.current.cells[key]}
          {@const value = results.get(key)}
          <div class="pocket-cell" role="gridcell" tabindex="-1"
            aria-label={`${key}${cell?.stamp ? ` ${cell.stamp}` : ""}: ${display(value ?? "") || "empty"}${cell?.input.startsWith("=") ? `, formula ${cell.input}` : ""}`}
            data-tint={cell?.tint} data-kind={kind(value)} data-formula={cell?.input.startsWith("=") ? "" : undefined}
            data-bounce={ui.bouncing.has(key) ? "" : undefined}>
            {#if cell?.stamp}<span class="pocket-stamp" aria-hidden="true">{cell.stamp}</span>{/if}
            <span class="pocket-value" title={isError(value ?? "") ? "This formula can't be worked out. Check its references." : undefined}>{display(value ?? "")}</span>
            {#if ui.bouncing.has(key)}<span class="pocket-sparkle" aria-hidden="true">✨</span>{/if}
          </div>
        {/each}
      </div>
    {/each}
  </div>
</article>
