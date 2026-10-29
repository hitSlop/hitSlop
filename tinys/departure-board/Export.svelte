<script lang="ts">
  import "@hitslop/tinys/base.css";
  import "./styles.css";
  import type { CaptureProps } from "@hitslop/tinys/svelte";
  import type schema from "./schema";
  import { boardView } from "./board";
  let { snapshot, preview = false }: CaptureProps<typeof schema> & { preview?: boolean } = $props();

  const view = $derived(boardView(snapshot));
</script>

<article class="board-shell export" aria-label="Exported departure board">
  <section class="board">
    <header class="head"><h1>{view.heading}</h1></header>
    <p class="labels" aria-hidden="true"><span>Time</span><span>Destination</span><span>Status</span></p>
    <ul class="rows">
      {#each (preview ? (view.rows).slice(0, 6) : view.rows) as { row, groups } (row.$id)}
        <li>
          <div class="row" data-status={row.status}>
            {#each groups as group (group.kind)}
              <span class="group {group.kind}">
                {#each group.cells as char, i (i)}<span class="flap static"><i>{char}</i></span>{/each}
              </span>
            {/each}
          </div>
        </li>
      {:else}
        <li class="empty"><p>No departures.</p></li>
      {/each}
    </ul>
  </section>
</article>
