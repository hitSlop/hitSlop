<script lang="ts">
  import doc from "./schema";
  import Chart from "./Chart.svelte";
  import { notebookView } from "./lab";

  const data = $derived(doc.current);
  const view = $derived(notebookView(data));
  const small = $derived(data.rows.length <= 12);
</script>

<article class="lab-page" aria-label="Exported figure">
  <h1>{data.title || "Untitled figure"}</h1>
  <div class="lab-figure"><Chart {data} width={598} height={380} /></div>
  {#if view.caption}<p class="lab-caption">{view.caption}</p>{/if}
  {#if small && data.rows.length}
    <table class="lab-data">
      <thead><tr>
        <th scope="col">{data.kind === "bar" ? "Category" : "#"}</th>
        <th scope="col">{view.xTitle || "x"}</th><th scope="col">{view.yTitle || "y"}</th>
        {#if view.hasBars}<th scope="col">&plusmn; y</th>{/if}
      </tr></thead>
      <tbody>
        {#each data.rows as row, i (row.$id)}
          <tr><td>{data.kind === "bar" ? row.label || row.x : i + 1}</td><td>{row.x}</td><td>{row.y}</td>{#if view.hasBars}<td>{row.err || ""}</td>{/if}</tr>
        {/each}
      </tbody>
    </table>
  {/if}
</article>
