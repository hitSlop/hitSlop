<script lang="ts">
  import doc from "./schema";
  import { cellKey, columnName, COLUMNS, display, isError, ROWS } from "./formula";
  import { cellLabel, gridTemplate, kindOf, sheetView, widthOf } from "./sheet";
  const { results } = $derived(sheetView(doc.current));
  const template = $derived(gridTemplate(column => widthOf(doc.current, column)));
</script>

<article class="pocket-export-view">
  <p class="pocket-eyebrow">Pocket Sheet</p>
  <h1>{doc.current.title || "Untitled sheet"}</h1>
  <div class="pocket-grid" style:grid-template-columns={template}
    role="grid" aria-readonly="true" aria-label={`${doc.current.title || "Pocket Sheet"} cells`}
    aria-rowcount={ROWS + 1} aria-colcount={COLUMNS + 1} tabindex="-1">
    <div class="pocket-row" role="row" style:grid-template-columns={template}>
      <span class="pocket-corner" role="columnheader" aria-label="Rows"></span>
      {#each Array.from({ length: COLUMNS }, (_, c) => c) as column}
        <span class="pocket-colhead" role="columnheader">{columnName(column)}</span>
      {/each}
    </div>
    {#each Array.from({ length: ROWS }, (_, r) => r) as row}
      <div class="pocket-row" role="row" style:grid-template-columns={template}>
        <span class="pocket-rowhead" role="rowheader">{row + 1}</span>
        {#each Array.from({ length: COLUMNS }, (_, c) => c) as column}
          {@const key = cellKey(column, row)}
          {@const cell = doc.current.cells[key]}
          {@const value = results.get(key)}
          <div class="pocket-cell" role="gridcell" tabindex="-1" aria-label={cellLabel(key, cell, value)}
            data-tint={cell?.tint} data-kind={kindOf(value)} data-formula={cell?.input.startsWith("=") ? "" : undefined}>
            {#if cell?.stamp}<span class="pocket-stamp" aria-hidden="true">{cell.stamp}</span>{/if}
            <span class="pocket-value" title={isError(value ?? "") ? "This formula can't be worked out. Check its references." : undefined}>{display(value ?? "")}</span>
          </div>
        {/each}
      </div>
    {/each}
  </div>
</article>

<style>
  .pocket-export-view .pocket-grid { display: block; }
  .pocket-export-view .pocket-row { display: grid; min-height: 28px; }
</style>
