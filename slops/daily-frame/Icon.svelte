<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  // A fanned stack of three prints, the newest on top with its date imprint.
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import { dayKey, frameView } from "./frames";
  import Photo from "./Photo.svelte";

  let { snapshot, attachmentURL }: CaptureProps<typeof schema> = $props();
  const view = $derived(frameView(snapshot, dayKey(new Date())));
  const prints = $derived(view.stack.slice(-3));
</script>

<div class="df-icon" aria-hidden="true">
  {#each prints as shown, at (shown.key)}
    <div class="df-icon-print" style={`--turn: ${[-11, 7, -2].slice(3 - prints.length)[at]}deg`}>
      <span class="df-icon-photo"><Photo {shown} url={attachmentURL} imprint={at === prints.length - 1} /></span>
    </div>
  {/each}
</div>
