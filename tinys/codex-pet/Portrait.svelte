<script lang="ts">
  import starterSheet from "./assets/bubu-spritesheet.webp";
  import { petView, type PetDocument } from "./pet";
  let { snapshot, label }: { snapshot: PetDocument; label?: string } = $props();
  const view = $derived(petView(snapshot));
</script>
<div class="pet-icon" aria-label={label} aria-hidden={label ? undefined : true}>
  <div class="pet-capture-card">
    {#if view.imported}
      <span class="pet-capture-mark">🐾</span><h1>{view.displayName}</h1><p>{view.stored?.name}</p><small>Custom pet · animation in tinys</small>
    {:else}
      <svg width="400" height="400" viewBox="0 0 192 208" aria-label="Bubu"><image href={starterSheet} x="0" y="0" width="1536" height="1872" /></svg>
    {/if}
  </div>
</div>
<style>
  .pet-capture-card { width: 464px; height: 464px; margin: 24px; padding: 20px; box-sizing: border-box; border: 10px solid #4b342b; border-radius: 52px; background: #fff1d9; color: #4b342b; display: flex; flex-direction: column; align-items: center; justify-content: center; text-align: center; gap: 12px; }
  .pet-capture-mark { font-size: 96px; } h1 { font: 700 38px/1.15 sans-serif; margin: 0; overflow-wrap: anywhere; } p { font: 18px/1.4 sans-serif; margin: 0; overflow-wrap: anywhere; } small { font: 14px/1.3 sans-serif; }
</style>
