<script lang="ts">
  import { Slider } from "bits-ui";
  import { sliderRange } from "./notebook";

  let { values, onpreview, oncommit }: {
    /** Each parameter named in the line, with the value the graph is using. */
    values: Record<string, number>;
    onpreview: (name: string, value: number) => void;
    oncommit: (name: string, value: number) => void;
  } = $props();

  const names = $derived(Object.keys(values));
  const shown = (value: number) => (Math.round(value * 100) / 100).toString().replace("-", "−");
</script>

{#if names.length}
  <div class="params" role="group" aria-label="Parameters">
    {#each names as name (name)}
      {@const value = values[name]!}
      {@const range = sliderRange(value)}
      <div class="param">
        <span class="param-name" aria-hidden="true">{name}</span>
        <Slider.Root
          type="single"
          {value}
          min={range.min}
          max={range.max}
          step={range.step}
          onValueChange={(next) => onpreview(name, next)}
          onValueCommit={(next) => oncommit(name, next)}
          class="param-slider"
        >
          <span class="param-track"><Slider.Range class="param-range" /></span>
          <Slider.Thumb index={0} class="param-thumb" aria-label={`Parameter ${name}`} />
        </Slider.Root>
        <output class="param-value">{shown(value)}</output>
      </div>
    {/each}
  </div>
{/if}
