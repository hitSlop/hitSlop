<script lang="ts" module>
  export type Particle = { id: number; symbol: string; left: number; size: number; delay: number; duration: number };
</script>

<script lang="ts">
  import { Progress } from "bits-ui";
  import doc from "./schema";

  let { level, percent, interactive, particles = [], oncelebrate = () => {}, onparticleend = () => {} }: {
    level: number;
    percent: number;
    interactive: boolean;
    particles?: Particle[];
    oncelebrate?: () => void;
    onparticleend?: (id: number) => void;
  } = $props();
</script>

<Progress.Root value={Math.min(percent, 100)} max={100} class="chamber" data-complete={percent >= 100} aria-label="Daily hydration progress">
  <div class="liquid" style:transform={`translateY(${100 - level}%)`} aria-hidden="true">
    <div class="waveCap" data-complete={percent >= 100} aria-hidden="true"></div>
  </div>
  {#if interactive && percent >= 100 && particles.length}
    <div class="confetti" aria-hidden="true" data-slop-export="hide">
      {#each particles as particle (particle.id)}
        <span class="particle" onanimationend={() => onparticleend(particle.id)} style="left:{particle.left}%;font-size:{particle.size}px;animation-delay:{particle.delay}s;animation-duration:{particle.duration}s">{particle.symbol}</span>
      {/each}
    </div>
  {/if}
  <div class="readout" aria-live="polite">
    {#if percent >= 100}
      {#if interactive}
        <button type="button" class="badge" data-slop-export="hide" aria-label="Goal reached, celebrate again" onclick={oncelebrate}>Goal reached</button>
      {:else}
        <span class="badge">Goal reached</span>
      {/if}
    {/if}
    <strong class="digits">{doc.current.current.toLocaleString()}</strong>
    <span class="unit">{doc.current.unit}</span>
    <span class="percent">{percent}% of daily goal</span>
  </div>
</Progress.Root>
