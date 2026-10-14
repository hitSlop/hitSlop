<script lang="ts">
  import type { CaptureProps } from "hitslop/svelte";
  import Check from "@lucide/svelte/icons/check";
  import type schema from "./schema";
  import { checklistView } from "./tasks";
  let { snapshot }: CaptureProps<typeof schema> = $props();
  const { marks } = $derived(checklistView(snapshot));
</script>

<div class="checklist-icon-tile" aria-hidden="true">
  <div class="checklist-icon-paper">
    {#each [0, 1, 2] as index}
      <div data-complete={index < marks}><span>{#if index < marks}<Check size={32} strokeWidth={3} />{/if}</span><i></i></div>
    {/each}
  </div>
</div>
