<script lang="ts">
  import type { Snippet } from "svelte";
  let { label, ratio = 1, kind = "focus", modes, detail }: {
    label: string;
    ratio?: number;
    kind?: "focus" | "rest";
    modes?: Snippet;
    detail?: Snippet;
  } = $props();
  const circumference = 2 * Math.PI * 103;
</script>

<div class="pom-dial" data-kind={kind}>
  <svg class="pom-dial-graphic" viewBox="0 0 260 260" aria-hidden="true">
    {#each Array(60) as _, index}
      <line class:major={index % 5 === 0} class="pom-dial-tick" x1="130" y1="12" x2="130" y2={index % 5 === 0 ? 23 : 17} transform={`rotate(${index * 6} 130 130)`} />
    {/each}
    <circle class="pom-dial-track" cx="130" cy="130" r="103" />
    <circle class="pom-dial-progress" cx="130" cy="130" r="103" transform="rotate(-90 130 130)" stroke-dasharray={circumference} stroke-dashoffset={(1 - ratio) * circumference} />
  </svg>
  <div class="pom-readout">
    {#if modes}{@render modes()}{:else}<p class="pom-static-mode">{kind === "focus" ? "Focus" : "Break"}</p>{/if}
    <output class="pom-digits" role="timer" aria-live="off" aria-label={`${kind === "focus" ? "Focus" : "Break"}: ${label} remaining`}>{label}</output>
    {#if detail}{@render detail()}{/if}
  </div>
</div>
