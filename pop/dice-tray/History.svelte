<script lang="ts">
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import { formatWhen, type RollView } from "./dice";

  // The score slip: newest roll first. The editor passes `onAgain`; the export does not.
  let { rolls, now, onAgain }: { rolls: RollView[]; now: number; onAgain?: (roll: RollView) => void } = $props();
</script>

<ol class="slip-list">
  {#each rolls as roll (roll.id)}
    <li class="slip-row">
      <span class="slip-total">{roll.total}</span>
      <span class="slip-main">
        <span class="slip-formula">{roll.formula}{#if roll.natural}<span class="slip-natural">Nat {roll.natural}</span>{/if}</span>
        <span class="slip-dice">{roll.breakdown}</span>
      </span>
      <span class="slip-when">{formatWhen(roll.at, now)}</span>
      {#if onAgain}
        <button class="slip-again" aria-label={`Roll ${roll.formula} again`} title="Roll again" onclick={() => onAgain(roll)}>
          <RotateCcw size={16} aria-hidden="true" />
        </button>
      {/if}
    </li>
  {:else}
    <li class="slip-empty">Rolls you make are kept here.</li>
  {/each}
</ol>
