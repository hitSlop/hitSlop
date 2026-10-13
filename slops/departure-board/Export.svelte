<script lang="ts">
  let { preview = false }: { preview?: boolean } = $props();
  import doc from "./schema";
  import { boardView } from "./board";

  const view = $derived(boardView(doc.current));
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
