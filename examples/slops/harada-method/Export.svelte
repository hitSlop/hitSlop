<script lang="ts">
import Check from "@lucide/svelte/icons/check";
import doc from "./schema";
import { CELLS, sheetView } from "./chart";
const { doneCount, deadlineText, isDone, textOf } = $derived(sheetView(doc.current));
</script>

<article class="exportPage" aria-label="Exported Open Window 64 chart">
      <header class="masthead">
        <div class="crest" aria-hidden="true"></div>
        <div>
          <p class="wordmark">Open Window 64</p>
          <h1 class="title">Harada Method</h1>
        </div>
        <div class="target">
          <span>Target</span>
          <strong>{deadlineText || "No date set"}</strong>
        </div>
        <div class="tally">
          <p class="tallyCount"><b>{doneCount}</b><small>/ 64</small></p>
          <div class="tallyBar" aria-hidden="true"><span class="tallyFill" style:width={`${(doneCount / 64) * 100}%`}></span></div>
          <p class="tallyLabel">Actions taken</p>
        </div>
      </header>

      <div class="exportSheet">
        {#each CELLS as cell (cell.row * 9 + cell.col)}
          <div
            class="cell"
            data-role={cell.role}
            data-x={cell.edgeX}
            data-y={cell.edgeY}
            data-done={cell.role === "action" && isDone(cell.theme, cell.action)}
          >
            <div class="write">{textOf(cell)}</div>
            {#if cell.role === "action" && isDone(cell.theme, cell.action)}
              <span class="tick" data-checkbox-root data-state="checked" aria-hidden="true"><Check size={8} strokeWidth={3} /></span>
            {/if}
          </div>
        {/each}
      </div>
    </article>
