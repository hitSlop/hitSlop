<script lang="ts">
  import "hitslop/base.css";
  import "./mathlive-static.css";
  import "./styles.css";
  import { previewText } from "../_shared/preview";
  import type { CaptureProps } from "hitslop/svelte";
  // The worked solution as a clean sheet: the same title block, numbered typeset lines,
  // notes, and each graph drawn as SVG from its saved window and parameters.
  import type schema from "./schema";
  import { notebookView, paramValues, plotCaption } from "./notebook";
  import StaticMath from "./StaticMath.svelte";
  import PlotFigure from "./PlotFigure.svelte";
  let { snapshot, preview = false }: CaptureProps<typeof schema> & { preview?: boolean } = $props();

  const view = $derived(notebookView(snapshot));
  const shown = (value: number) => (Math.round(value * 100) / 100).toString().replace("-", "−");
</script>

<main class="pad pad-export">
  <header class="title-block">
    <div class="title-cell">
      <span class="block-label">Problem</span>
      <h1 class="title">{view.title}</h1>
    </div>
    <div class="block-cell"><span class="block-label">Lines</span><span class="block-value">{view.equations}</span></div>
    <div class="block-cell"><span class="block-label">Sheet</span><span class="block-value">1 of 1</span></div>
  </header>
  <div class="sheet">
    <ol class="lines">
      {#each (preview ? (view.lines).slice(0, 4) : view.lines) as line (line.cell.$id)}
        {@const cell = line.cell}
        {@const values = line.plot?.ok ? paramValues(cell, line.plot.params) : {}}
        <li class="line" data-kind={cell.kind}>
          <div class="line-body" data-graphed={line.graphed || undefined}>
            {#if cell.kind === "note"}
              <p class="note">{previewText(cell.note, preview, 160)}</p>
            {:else}
              {#if line.graphed && cell.kind === "graph"}
                <figure class="export-graph" data-size="large">
                  <PlotFigure plot={line.plot} params={values} xMin={cell.xMin} xMax={cell.xMax} width={520} height={220} label={`Graph ${line.number}`} />
                </figure>
              {/if}
              <div class="equation"><StaticMath latex={cell.latex} /></div>
              {#if line.graphed && cell.kind === "equation"}
                <figure class="export-graph">
                  <PlotFigure plot={line.plot} params={values} xMin={cell.xMin} xMax={cell.xMax} width={260} height={150} label={`Graph of equation ${line.number}`} />
                </figure>
              {/if}
              {#if line.graphed}
                {#if line.plot && !line.plot.ok}<p class="caption">Can't plot this: {line.plot.reason}</p>{/if}
                {#if Object.keys(values).length}
                  <p class="export-params">{Object.entries(values).map(([name, value]) => `${name} = ${shown(value)}`).join(",  ")}</p>
                {/if}
                {#if line.plot?.ok && plotCaption(line.plot)}<p class="caption">{plotCaption(line.plot)}</p>{/if}
              {/if}
            {/if}
          </div>
          <div class="line-margin">{#if line.number}<span class="eq-number">({line.number})</span>{/if}</div>
        </li>
      {/each}
    </ol>
  </div>
</main>
