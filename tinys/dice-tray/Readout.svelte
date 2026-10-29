<script lang="ts">
  import type { Snippet } from "svelte";
  import type { RollView } from "./dice";

  // The brass plate: the last roll's total, what was rolled and how it added up.
  let { roll, rolling = false, title }: { roll: RollView | undefined; rolling?: boolean; title: Snippet } = $props();
</script>

<header class="plate">
  <div class="plate-text">
    {@render title()}
    {#if roll}
      <p class="plate-formula" data-long={roll.formula.length > 20}>{roll.formula}</p>
      <p class="plate-breakdown">
        {#if rolling}Rolling…{:else}{roll.breakdown}{#if roll.natural}<span class="plate-natural">Natural {roll.natural}</span>{/if}{/if}
      </p>
    {:else}
      <p class="plate-formula">No rolls yet</p>
      <p class="plate-breakdown">Pick dice below and roll.</p>
    {/if}
  </div>
  <p class="plate-total" data-rolling={rolling} aria-hidden="true">{roll && !rolling ? roll.total : rolling ? "…" : "–"}</p>
</header>
