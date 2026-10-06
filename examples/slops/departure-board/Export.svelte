<script lang="ts">
  import doc from "./schema";
  import { cells, clean, statusLabel, widths } from "./board";
</script>

<article class="board-shell export" aria-label="Exported departure board">
  <section class="board">
    <header class="head"><h1>{clean(doc.current.title, 24) || "DEPARTURES"}</h1></header>
    <p class="labels" aria-hidden="true"><span>Time</span><span>Destination</span><span>Status</span></p>
    <ul class="rows">
      {#each doc.current.rows as row (row.$id)}
        <li>
          <div class="row" data-status={row.status}>
            {#each [["time", row.time], ["text", row.text], ["status", statusLabel[row.status]]] as [kind, text] (kind)}
              <span class="group {kind}">
                {#each cells(text!, widths[kind as keyof typeof widths]) as char, i (i)}<span class="flap static"><i>{char}</i></span>{/each}
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
