<script lang="ts">
  // How far the list is, in words and as a track: shared by the editor and its export.
  let { total, finished, fill, live = false }: {
    total: number;
    finished: number;
    /** The track's fill, 0 to 100. */
    fill: number;
    /** Announce changes: the editor's progress is live, the export's is not. */
    live?: boolean;
  } = $props();
</script>

<div class="checklist-progress">
  <span aria-live={live ? "polite" : undefined}
    >{total && finished === total ? "All done. Nicely done." : `${total - finished} left to do`}</span
  >
  <span>{finished} / {total} done</span>
</div>
<div class="checklist-track" aria-hidden="true">
  <div style:transform={`scaleX(${fill / 100})`}></div>
</div>
