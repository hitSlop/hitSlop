<script lang="ts">
  import type { Countdown } from "./schema";
  import type { CountdownState } from "./countdown";
  import { displayDate } from "./countdown";

  let {
    data,
    state,
    onEdit,
  }: { data: Countdown; state: CountdownState; onEdit?: () => void } = $props();
</script>

<section class="calendar" aria-label={state.accessible}>
  <span class="binding" style:left="22%" aria-hidden="true"></span>
  <span class="binding" style:right="22%" aria-hidden="true"></span>
  <h1 class="title">{data.title || "Something to look forward to"}</h1>
  <div class="readout">
    <strong class="number" data-long={state.value.length > 3}>{state.value}</strong>
    {#if state.kind !== "today"}<span class="unit">{state.unit}</span>{/if}
    <p class="detail">{state.detail}</p>
  </div>
  {#if onEdit}
    <button class="dateButton" onclick={onEdit} aria-label="Edit event date and time">
      {displayDate(data.targetDate, data.targetTime)} ↗
    </button>
  {:else}
    <div class="dateButton">{displayDate(data.targetDate, data.targetTime)}</div>
  {/if}
</section>
