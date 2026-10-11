<script lang="ts">
  // One frame's picture: the saved photo, or the drawn sample, with the film-camera
  // date imprint in the corner.
  import { attachments } from "hitslop/svelte";
  import SampleArt from "./SampleArt.svelte";
  import { dayLabel, stamp, type Shown } from "./frames";

  let { shown, imprint = true, lazy = false, onerror }: {
    shown: Shown;
    imprint?: boolean;
    lazy?: boolean;
    onerror?: () => void;
  } = $props();
</script>

<span class="df-photo">
  {#if shown.photo}
    <img src={attachments.url(shown.photo)} alt={`Frame from ${dayLabel(shown.date)}${shown.note ? `: ${shown.note}` : ""}`}
      loading={lazy ? "lazy" : "eager"} decoding="async" draggable="false" {onerror} />
  {:else}
    <SampleArt growth={shown.growth} />
  {/if}
  {#if imprint}<span class="df-imprint" aria-hidden="true">{stamp(shown.date)}</span>{/if}
</span>
