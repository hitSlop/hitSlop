<script lang="ts">
  import { courseLabel, questLabel } from "./map";
  import { daysBetween, hp, when, type Course, type Quest } from "./quest";

  let { boss, today }: { boss: { course: Course; quest: Quest } | undefined; today: string } = $props();
</script>

<div class="sq-readout" aria-live="polite">
  {#if boss}
    {@const days = daysBetween(today, boss.quest.due)}
    {@const bossHp = hp(boss.quest)}
    <span class="sq-readout-eyebrow">Next boss</span>
    <strong>{courseLabel(boss.course)} {questLabel(boss.quest)}</strong>
    <span class="sq-readout-meta">{when(days)} · <b>{bossHp === null ? "HP unavailable" : `${Math.round((bossHp / boss.quest.maxHp) * 100)}% HP`}</b></span>
  {:else}
    <span class="sq-readout-eyebrow">Next boss</span>
    <strong>None in sight</strong>
    <span class="sq-readout-meta">Every boss is down. Touch grass.</span>
  {/if}
</div>
